use {
    illume::{
        CreateEncoderError, Device, Encoder, Fence, Queue, QueueId, Semaphore,
    },
    maybe_sync::Mutex,
    std::{
        collections::HashMap,
        future::Future,
        ops::{Deref, DerefMut},
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    },
};

/// Worker allows to run commands for GPU in async manner.
/// Each comman returns `Future` implmenentation that resolves when operation is
/// complete. This approach is best suitable for non-periodic jobs like:
/// * vertex and images loading
/// * mip level building
/// * acceleration structure building,
#[derive(Debug)]
pub struct Worker {
    jobs: HashMap<QueueId, Vec<Arc<Job>>>,
}

impl Worker {
    /// Create new `Worker`.
    pub fn new() -> Self {
        Worker {
            jobs: HashMap::new(),
        }
    }

    /// Returns `AsyncEncoder` - a special `Encoder` wrapper.
    /// `AsyncEncoder` returns a future upon flushing
    /// that will be resolved when all encoded commands complete.
    pub fn encode<'a>(
        &'a mut self,
        queue: &'a mut Queue,
        device: &Device,
    ) -> Result<AsyncEncoder<'a>, CreateEncoderError> {
        let mut queue_jobs =
            self.jobs.entry(queue.id()).or_insert_with(|| Vec::new());

        Ok(AsyncCommandBuffer {
            command_buffer: queue.create_command_buffer()?,
            task: Arc::new(Job {
                fence: device.create_fence()?,
                waker: Mutex::new(None),
                ready: AtomicBool::new(false),
            }),
            queue_jobs,
            queue,
        })
    }

    /// Check all jobs for completion.
    /// Completed jobs will wake async tasks that wait on the handler.
    /// This function should be called periodically.
    pub fn check(&mut self, device: &Device) {
        for queue_jobs in self.jobs.values_mut() {
            for i in 0..queue_jobs.len() {
                if !device.is_fence_signalled(&queue_jobs[i].fence) {
                    for ready_task in queue_jobs.drain(..i) {
                        ready_task.ready.store(true, Ordering::Release);
                        if let Some(waker) = ready_task.waker.lock().take() {
                            waker.wake();
                        }
                    }
                }
            }
        }
    }
}

/// Handle for commands that are executed on GPU.
#[derive(Debug)]
pub struct CommandsHandle {
    task: Arc<Job>,
}

impl Future for CommandsHandle {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<()> {
        if self.task.ready.load(Ordering::Acquire) {
            Poll::Ready(())
        } else {
            let waker = ctx.waker();
            let mut lock = self.task.waker.lock();
            if lock.as_ref().map_or(true, |w| !w.will_wake(waker)) {
                *lock = Some(waker.clone());
            }
            drop(lock);
            if self.task.ready.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

#[derive(Debug)]
pub struct AsyncEncoder<'a> {
    encoder: Encoder<'a>,
    task: Arc<Job>,
    queue: &'a mut Queue,
    queue_jobs: &'a mut Vec<Arc<Job>>,
}

impl AsyncEncoder<'_> {
    pub fn flush(self) -> CommandsHandle {
        self.queue.submit(
            &[],
            self.encoder.flush(),
            &[],
            Some(&self.task.fence),
        );
        self.queue_jobs.push(self.task.clone());
        CommandsHandle { task: self.task }
    }
}

impl<'a> Deref for AsyncEncoder<'a> {
    type Target = Encoder<'a>;

    fn deref(&self) -> &Encoder<'a> {
        &self.encoder
    }
}

impl<'a> DerefMut for AsyncEncoder<'a> {
    fn deref_mut(&mut self) -> &mut Encoder<'a> {
        &mut self.encoder
    }
}

#[derive(Debug)]
struct Job {
    ready: AtomicBool,
    fence: Fence,
    waker: Mutex<Option<Waker>>,
}

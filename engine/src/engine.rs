use {
    crate::config::{AssetSource, Config},
    cfg_if::cfg_if,
    color_eyre::Report,
    eyre::WrapErr as _,
    goods::Cache,
    hecs::World,
    std::{
        cell::Cell,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll},
        time::Instant,
    },
    tokio::{runtime::Runtime, task::yield_now},
    winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
        window::{Theme, Window, WindowBuilder, WindowId},
    },
};

pub use winit::event::{
    AxisId, DeviceEvent, DeviceId, ElementState, KeyboardInput, ModifiersState,
    MouseButton, MouseScrollDelta, Touch, TouchPhase,
};

struct Shared {
    event_loop_ptr: Cell<*const EventLoopWindowTarget<()>>,
    next_event: Cell<Option<Event<'static, ()>>>,
    waiting_for_event: Cell<bool>,
}

/// Root data structure for the game engine.
pub struct Engine {
    pub world: World,
    pub assets: Cache<String>,
    schedule: Vec<Box<dyn FnMut(&mut World)>>,
    shared: Rc<Shared>,
}

impl Engine {
    pub fn build_window(
        &mut self,
        builder: WindowBuilder,
    ) -> Result<Window, Report> {
        let elwt = self.shared.event_loop_ptr.get();
        if elwt.is_null() {
            unreachable!()
        }

        let elwt = unsafe {
            // This block can be executed only within winit's event loop
            // callback. when closure provided to `Engine::run` is
            // polled, or on initial call to that closure.
            // Because it takes `&mut self` and `Engine is not `Send` it cannot
            // be sent to another thread.
            // This function is not async so it is not possible that reference
            // we creating here will be preserved over yielding.
            &*elwt
        };

        let window = builder.build(elwt)?;
        Ok(window)
    }

    /// Adds a system to this engine.
    pub fn add_system<A, S>(&mut self, system: S) -> &mut Self
    where
        S: FnMut(&mut World) + 'static,
    {
        self.schedule.push(Box::new(system));
        self
    }

    /// Asynchronously wait for next event.
    pub async fn next(&mut self) -> Event<'static, ()> {
        self.shared.waiting_for_event.set(true);
        // let event = self.events.recv_async().await;

        let event = loop {
            if let Some(event) = self.shared.next_event.take() {
                break event;
            }
            yield_now().await;
        };

        self.shared.waiting_for_event.set(false);
        event
    }

    /// Runs an instance of an engine.
    /// This function neven returns on success.
    /// Instead it calls provided closure with create engine instance
    /// and drive it to completion.
    /// Along with polling winit's event-loop for window events.
    pub fn run<F, A>(closure: F) -> Result<(), Report>
    where
        F: FnOnce(Self) -> A,
        A: Future<Output = Result<(), Report>> + 'static,
    {
        let mut runtime = Runtime::new()?;

        // Setup basic logging first to capture all following initializing
        // errors.
        runtime.block_on(Self::init_logger())?;

        let config = runtime.block_on(Self::load_config())?;

        let registry = config
            .sources
            .iter()
            .fold(goods::RegistryBuilder::new(), |builder, source| match source {
                AssetSource::FileSystem { path } => {
                    cfg_if! {
                        if #[cfg(target_arch = "wasm32")] {
                            tracing::error!("FileSystem asset source with path '{}' ignored on WASM target", path.display());
                            Ok(builder)
                        } else {
                            let path = match std::env::current_dir() {
                                Ok(cd) => { cd.join(path) }
                                Err(err) => {
                                    tracing::error!("Failed to fetch current dir: {}", err);
                                    path.clone()
                                }
                            };
                            builder.with(goods::FileSource::new(path))
                        }
                    }
                }
            });

        let assets = Cache::new(
            registry.build(),
            goods::Tokio(runtime.handle().clone()),
        );

        let shared = Rc::new(Shared {
            event_loop_ptr: Cell::new(std::ptr::null()),
            next_event: Cell::new(None),
            waiting_for_event: Cell::new(false),
        });

        let now = Instant::now();

        let engine = Engine {
            assets,
            schedule: Vec::new(),
            world: World::new(),
            shared: shared.clone(),
        };

        let event_loop = EventLoop::new();

        shared.event_loop_ptr.set(&*event_loop);
        let mut app_opt = Some(Box::pin(closure(engine)));
        shared.event_loop_ptr.set(std::ptr::null());

        // Here goes magic
        event_loop.run(move |event, el, flow| {
            tracing::trace!("Event {:#?}", event);

            if let Some(app) = &mut app_opt {
                // Set event. Excluding an event bound to a lifetime.
                let old = match event.to_static() {
                    Some(event) => {
                        shared.next_event.replace(Some(event))
                    }
                    None => {
                        shared.next_event.take()
                    }
                };

                assert!(old.is_none(), "Control flow must not return to event loop until event is consumed by the application");

                // Set pointer. We ensure it is always valid while
                // non-null.
                shared.event_loop_ptr.set(el);

                // Poll closure only once.
                if let Poll::Ready(result) =
                    runtime.block_on(AppEventWaitFuture {
                        app: app.as_mut(),
                        ready: &shared.waiting_for_event,
                    })
                {
                    // No place where we could return this error.
                    // log and panic are only options.
                    if let Err(err) = result {
                        tracing::error!("Error: {}", err);
                    }

                    // Exit when closure resolves.
                    *flow = ControlFlow::Exit;
                    app_opt = None;
                } else {
                    // *flow = ControlFlow::Wait;
                    *flow = ControlFlow::Poll;
                }

                // Unset event loop before it is invalidated.
                shared.event_loop_ptr.set(std::ptr::null());
            } else {
                *flow = ControlFlow::Exit;
            }
        })
    }

    async fn init_logger() -> Result<(), Report> {
        tracing_subscriber::fmt::init();
        Ok(())
    }

    async fn load_config() -> Result<Config, Report> {
        tracing::info!("Running at {}", std::env::current_dir()?.display());

        // Now load config.
        let config = Config::load_default().await?;
        tracing::info!("Config loaded: {:?}", config);
        Ok(config)
    }
}

/// Future that polls main application future
/// until it finishes or awaits in `Engine::next` function.
#[must_use = "futures/streams/sinks do nothing unless you `.await` or poll them"]
struct AppEventWaitFuture<'a, A> {
    app: Pin<&'a mut A>,
    ready: &'a Cell<bool>,
}

impl<'a, A> Future for AppEventWaitFuture<'a, A>
where
    A: Future<Output = Result<(), Report>>,
{
    type Output = Poll<Result<(), Report>>;

    fn poll(
        self: Pin<&mut Self>,
        ctx: &mut Context,
    ) -> Poll<Poll<Result<(), Report>>> {
        let this = self.get_mut();
        let poll = Future::poll(this.app.as_mut(), ctx);
        match poll {
            Poll::Ready(result) => Poll::Ready(Poll::Ready(result)),
            Poll::Pending if this.ready.get() => Poll::Ready(Poll::Pending),
            Poll::Pending => Poll::Pending,
        }
    }
}

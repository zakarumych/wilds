use crate::{
    convert::ToErupt as _,
    device::Device,
    encode::{CommandBuffer, Encoder},
    fence::Fence,
    out_of_host_memory,
    semaphore::Semaphore,
    stage::PipelineStageFlags,
    surface::SwapchainImage,
    OutOfMemory,
};
use erupt::{
    extensions::khr_swapchain::{
        KhrSwapchainDeviceLoaderExt as _, PresentInfoKHR,
    },
    vk1_0::{self, Vk10DeviceLoaderExt as _},
};
use smallvec::SmallVec;
use std::{
    error::Error,
    fmt::{self, Debug},
};

/// Capability a queue may have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum Capability {
    Transfer,
    Compute,
    Graphics,
}

bitflags::bitflags! {
    /// Queue capability flags.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct QueueCapabilityFlags: u32 {
        const TRANSFER  = 0b001;
        const COMPUTE   = 0b010;
        const GRAPHICS  = 0b100;
    }
}

impl QueueCapabilityFlags {
    /// Check if queue with those flags supports specified capability.
    pub fn supports(&self, other: Capability) -> bool {
        match other {
            Capability::Transfer => self.contains(Self::TRANSFER),
            Capability::Compute => self.contains(Self::COMPUTE),
            Capability::Graphics => self.contains(Self::GRAPHICS),
        }
    }

    /// Check if queue with those flags supports specified capability.
    pub fn supports_graphics(&self) -> bool {
        self.contains(Self::GRAPHICS)
    }

    /// Check if queue with those flags supports specified capability.
    pub fn supports_compute(&self) -> bool {
        self.contains(Self::COMPUTE)
    }
}

/// Information about one queue family.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct FamilyInfo {
    /// Supported capabilities.
    /// All queues of one family have same set of capabilities.
    pub capabilities: QueueCapabilityFlags,

    /// Maximum number of queues from this family that can be created.
    pub count: usize,
}

/// Family of queues created togther with device.
#[derive(Debug)]
pub struct Family {
    pub capabilities: QueueCapabilityFlags,
    pub queues: Vec<Queue>,
}

impl Family {
    pub fn supports(&self, capability: Capability) -> bool {
        self.capabilities.supports(capability)
    }

    pub fn take(&mut self, count: usize) -> impl Iterator<Item = Queue> + '_ {
        std::iter::from_fn(move || self.queues.pop()).take(count)
    }
}

/// Trait for querying command queues.
pub trait QueuesQuery {
    type Error: Error + 'static;
    type Queues;
    type Query: AsRef<[(usize, usize)]>;
    type Collector;

    fn query(
        self,
        families: &[FamilyInfo],
    ) -> Result<(Self::Query, Self::Collector), Self::Error>;

    fn collect(
        collector: Self::Collector,
        families: Vec<Family>,
    ) -> Self::Queues;
}

#[derive(Clone, Copy, Debug)]
pub struct QueuesQueryClosure<F>(pub F);

impl<F, Q, E> QueuesQuery for QueuesQueryClosure<F>
where
    F: FnOnce(&[FamilyInfo]) -> Result<Q, E>,
    Q: IntoIterator<Item = (usize, usize)>,
    E: Error + 'static,
{
    type Collector = fn(Vec<Family>) -> Vec<Family>;
    type Error = E;
    type Query = Vec<(usize, usize)>;
    type Queues = Vec<Family>;

    fn query(
        self,
        families: &[FamilyInfo],
    ) -> Result<(Self::Query, Self::Collector), E> {
        Ok((
            (self.0)(families)?.into_iter().collect(),
            std::convert::identity,
        ))
    }

    fn collect(
        collector: Self::Collector,
        families: Vec<Family>,
    ) -> Self::Queues {
        collector(families)
    }
}

/// Query only one queue with specified capabilities.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct SingleQueueQuery(QueueCapabilityFlags);

impl SingleQueueQuery {
    pub const COMPUTE: Self = SingleQueueQuery(QueueCapabilityFlags::COMPUTE);
    pub const GENERAL: Self =
        SingleQueueQuery(QueueCapabilityFlags::from_bits_truncate(0b11));
    pub const GRAPHICS: Self = SingleQueueQuery(QueueCapabilityFlags::GRAPHICS);
    pub const TRANSFER: Self = SingleQueueQuery(QueueCapabilityFlags::TRANSFER);
}

/// Could not find a queue with specified capabilities.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct QueueNotFound(QueueCapabilityFlags);

impl std::fmt::Display for QueueNotFound {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            fmt,
            "Could not find a queue with following capabilities: {:?}",
            self.0,
        )
    }
}

impl std::error::Error for QueueNotFound {}

impl QueuesQuery for SingleQueueQuery {
    type Collector = usize;
    type Error = QueueNotFound;
    type Query = [(usize, usize); 1];
    type Queues = Queue;

    fn query(
        self,
        families: &[FamilyInfo],
    ) -> Result<([(usize, usize); 1], usize), QueueNotFound> {
        for (index, family) in families.iter().enumerate() {
            if family.count > 0 && family.capabilities.contains(self.0) {
                return Ok(([(index, 1)], index));
            }
        }

        Err(QueueNotFound(self.0))
    }

    fn collect(index: usize, mut families: Vec<Family>) -> Queue {
        families.remove(index).queues.remove(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct QueueId {
    pub family: usize,
    pub index: usize,
}

pub struct Queue {
    handle: vk1_0::Queue,
    pool: vk1_0::CommandPool,
    device: Device,
    id: QueueId,
    capabilities: QueueCapabilityFlags,
}

impl Debug for Queue {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("Queue")
                .field("handle", &self.handle)
                .field("id", &self.id)
                .field("capabilities", &self.capabilities)
                .field("device", &self.device)
                .finish()
        } else {
            write!(fmt, "{:p}", self.handle)
        }
    }
}

impl Queue {
    pub(crate) fn new(
        handle: vk1_0::Queue,
        pool: vk1_0::CommandPool,
        device: Device,
        id: QueueId,
        capabilities: QueueCapabilityFlags,
    ) -> Self {
        Queue {
            handle,
            device,
            pool,
            id,
            capabilities,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateEncoderError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
}

impl Queue {
    pub fn id(&self) -> QueueId {
        self.id
    }

    #[tracing::instrument]
    pub fn create_encoder(
        &mut self,
    ) -> Result<Encoder<'static>, CreateEncoderError> {
        if self.pool == vk1_0::CommandPool::null() {
            self.pool = unsafe {
                self.device.logical().create_command_pool(
                    &vk1_0::CommandPoolCreateInfo::default()
                        .builder()
                        .flags(
                            vk1_0::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                        )
                        .queue_family_index(self.id.family as u32),
                    None,
                    None,
                )
            }
            .result()
            .map_err(create_encoder_error_from_erupt)?;
        }

        assert_ne!(self.pool, vk1_0::CommandPool::null());

        let mut buffers = unsafe {
            self.device.logical().allocate_command_buffers(
                &vk1_0::CommandBufferAllocateInfo::default()
                    .builder()
                    .command_pool(self.pool)
                    .level(vk1_0::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .result()
        .map_err(create_encoder_error_from_erupt)?;

        let cbuf = CommandBuffer::new(
            buffers.remove(0),
            self.id,
            self.device.downgrade(),
        );

        Ok(Encoder::new(cbuf, self.capabilities))
    }

    #[tracing::instrument]
    pub fn submit(
        &mut self,
        wait: &[(PipelineStageFlags, Semaphore)],
        cbuf: CommandBuffer,
        signal: &[Semaphore],
        fence: Option<&Fence>,
    ) {
        assert_eq!(self.id, cbuf.queue());
        let cbuf = cbuf.handle(&self.device);

        // FIXME: Check semaphore states.
        let (wait_stages, wait_semaphores): (
            SmallVec<[_; 8]>,
            SmallVec<[_; 8]>,
        ) = wait
            .iter()
            .map(|(ps, sem)| (ps.to_erupt(), sem.handle(&self.device)))
            .unzip();

        let signal_semaphores: SmallVec<[_; 8]> =
            signal.iter().map(|sem| sem.handle(&self.device)).collect();

        unsafe {
            self.device
                .logical()
                .queue_submit(
                    self.handle,
                    &[vk1_0::SubmitInfo::default()
                        .builder()
                        .wait_semaphores(&wait_semaphores)
                        .wait_dst_stage_mask(&wait_stages)
                        .signal_semaphores(&signal_semaphores)
                        .command_buffers(&[cbuf])],
                    fence.map_or(vk1_0::Fence::null(), |f| {
                        f.handle(&self.device)
                    }),
                )
                .expect("TODO: Handle queue submit error")
        };
    }

    #[tracing::instrument]
    pub fn submit_no_semaphores(
        &mut self,
        buffer: CommandBuffer,
        fence: Option<&Fence>,
    ) {
        self.submit(&[], buffer, &[], fence);
    }

    #[tracing::instrument]
    pub fn present(&mut self, image: SwapchainImage) {
        // FIXME: Check semaphore states.
        assert!(
            self.device.logical().khr_swapchain.is_some(),
            "Should be enabled given that there is a Swapchain"
        );

        let swapchain_image_info = &image.info();

        assert!(
            image.supported_families(&self.device)[self.id.family as usize],
            "Family `{}` does not support presentation to swapchain `{:?}`",
            self.id.family,
            image
        );

        let mut result = vk1_0::Result::SUCCESS;

        unsafe {
            self.device.logical().queue_present_khr(
                self.handle,
                &PresentInfoKHR::default()
                    .builder()
                    .wait_semaphores(&[swapchain_image_info
                        .signal
                        .handle(&self.device)])
                    .swapchains(&[image.handle(&self.device)])
                    .image_indices(&[image.index() as u32])
                    .results(std::slice::from_mut(&mut result)),
            )
        }
        .expect("TODO: Handle present errors");
    }

    #[tracing::instrument]
    pub fn wait_for_idle(&self) {
        // FIXME: Handle DeviceLost error.
        unsafe {
            self.device
                .logical()
                .queue_wait_idle(self.handle)
                .expect("Device lost")
        }
    }
}

pub(crate) fn create_encoder_error_from_erupt(
    err: vk1_0::Result,
) -> CreateEncoderError {
    match err {
        vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => out_of_host_memory(),
        vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            CreateEncoderError::OutOfMemory {
                source: OutOfMemory,
            }
        }
        _ => CreateEncoderError::Other {
            source: Box::new(err),
        },
    }
}

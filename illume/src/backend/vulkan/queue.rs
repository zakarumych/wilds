use {
    super::{convert::ToErupt as _, device::Device, swapchain::SwapchainImage},
    crate::{
        encode::{CommandBuffer, Encoder},
        fence::Fence,
        out_of_host_memory,
        queue::*,
        semaphore::Semaphore,
        stage::PipelineStageFlags,
        OutOfMemory,
    },
    erupt::{extensions::khr_swapchain::PresentInfoKHR, vk1_0},
    smallvec::SmallVec,
    std::fmt::{self, Debug},
};

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
    #[error(transparent)]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Function returned unexpected error code: {result}")]
    UnexpectedVulkanError { result: vk1_0::Result },
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
                        .into_builder()
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
                    .into_builder()
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
                        .into_builder()
                        .wait_semaphores(&wait_semaphores)
                        .wait_dst_stage_mask(&wait_stages)
                        .signal_semaphores(&signal_semaphores)
                        .command_buffers(&[cbuf])],
                    fence.map(|f| f.handle(&self.device)),
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
            self.device.logical().enabled.khr_swapchain,
            "Should be enabled given that there is a Swapchain"
        );

        let swapchain_image_info = image.info();

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
                    .into_builder()
                    .wait_semaphores(&[swapchain_image_info
                        .signal
                        .handle(&self.device)])
                    .swapchains(&[image.handle(&self.device)])
                    .image_indices(&[*image.index(&self.device)])
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
        _ => CreateEncoderError::UnexpectedVulkanError { result: err },
    }
}

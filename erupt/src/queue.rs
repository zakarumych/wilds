use crate::{
    command::EruptCommandBuffer, convert::ToErupt as _, device::EruptDevice,
    handle::EruptResource as _,
};
use erupt::{
    extensions::khr_swapchain::{
        KhrSwapchainDeviceLoaderExt as _, PresentInfoKHR,
    },
    vk1_0::{self, Vk10DeviceLoaderExt as _},
};
use illume::{
    out_of_host_memory, CommandBuffer, CommandBufferTrait, CreateEncoderError,
    Fence, OutOfMemory, PipelineStageFlags, QueueTrait, Semaphore,
    SwapchainImage,
};
use smallvec::SmallVec;
use std::{
    fmt::{self, Debug},
    sync::Arc,
};

pub(super) struct EruptQueue {
    pub(super) queue: vk1_0::Queue,
    pub(super) family: u32,
    pub(super) index: u32,
    pub(super) device: Arc<EruptDevice>,
    pub(super) pool: vk1_0::CommandPool,
}

impl Debug for EruptQueue {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("EruptQueue")
                .field("queue", &self.queue)
                .field("device", &self.device)
                .finish()
        } else {
            Debug::fmt(&self.queue, fmt)
        }
    }
}

impl QueueTrait for EruptQueue {
    fn create_command_buffer(
        &mut self,
    ) -> Result<Box<dyn CommandBufferTrait>, CreateEncoderError> {
        if self.pool == vk1_0::CommandPool::null() {
            self.pool = unsafe {
                self.device.logical.create_command_pool(
                    &vk1_0::CommandPoolCreateInfo::default()
                        .builder()
                        .flags(
                            vk1_0::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                        )
                        .queue_family_index(self.family),
                    None,
                    None,
                )
            }
            .result()
            .map_err(create_encoder_error_from_erupt)?;
        }

        assert_ne!(self.pool, vk1_0::CommandPool::null());

        let buffers = unsafe {
            self.device.logical.allocate_command_buffers(
                &vk1_0::CommandBufferAllocateInfo::default()
                    .builder()
                    .command_pool(self.pool)
                    .level(vk1_0::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .result()
        .map_err(create_encoder_error_from_erupt)?;

        Ok(Box::new(EruptCommandBuffer {
            handle: buffers[0],
            device: Arc::downgrade(&self.device),
            family: self.family,
            queue: self.index,
            recording: false,
        }))
    }

    fn submit(
        &mut self,
        wait: &[(PipelineStageFlags, Semaphore)],
        buffer: CommandBuffer,
        signal: &[Semaphore],
        fence: Option<&Fence>,
    ) {
        let buffer = buffer.downcast::<EruptCommandBuffer>();

        // FIXME: Check semaphore states.
        let (wait_stages, wait_semaphores): (
            SmallVec<[_; 8]>,
            SmallVec<[_; 8]>,
        ) = wait
            .iter()
            .map(|(ps, sem)| {
                (ps.to_erupt(), sem.erupt_ref(&*self.device).handle)
            })
            .unzip();

        let signal_semaphores: SmallVec<[_; 8]> = signal
            .iter()
            .map(|sem| sem.erupt_ref(&*self.device).handle)
            .collect();

        unsafe {
            self.device
                .logical
                .queue_submit(
                    self.queue,
                    &[vk1_0::SubmitInfo::default()
                        .builder()
                        .wait_semaphores(&wait_semaphores)
                        .wait_dst_stage_mask(&wait_stages)
                        .signal_semaphores(&signal_semaphores)
                        .command_buffers(&[buffer.handle])],
                    fence.map_or(vk1_0::Fence::null(), |f| {
                        f.erupt_ref(&*self.device).handle
                    }),
                )
                .expect("TODO: Handle queue submit error")
        };
    }

    fn present(&mut self, image: SwapchainImage) {
        // FIXME: Check semaphore states.
        assert!(
            self.device.logical.khr_swapchain.is_some(),
            "Should be enabled given that there is a Swapchain"
        );

        let swapchain_image = image.erupt_ref(&*self.device);
        let swapchain_image_info = image.info();

        assert!(
            swapchain_image.supported_families[self.family as usize],
            "Family does not support presentation to that surface"
        );

        let mut result = vk1_0::Result::SUCCESS;

        unsafe {
            self.device.logical.queue_present_khr(
                self.queue,
                &PresentInfoKHR::default()
                    .builder()
                    .wait_semaphores(&[swapchain_image_info
                        .signal
                        .erupt_ref(&*self.device)
                        .handle])
                    .swapchains(&[swapchain_image.swapchain])
                    .image_indices(&[swapchain_image.index])
                    .results(std::slice::from_mut(&mut result)),
            )
        }
        .expect("TODO: Handle present errors");
    }

    fn wait_for_idle(&self) {
        // FIXME: Handle DeviceLost error.
        unsafe {
            self.device
                .logical
                .queue_wait_idle(self.queue)
                .expect("Device lost")
        }
    }
}

pub(super) fn create_encoder_error_from_erupt(
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

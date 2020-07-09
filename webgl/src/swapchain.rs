use crate::{device::WebGlDevice, handle::*, JsError, TypeError};
use illume::{
    command::CommandBufferTrait,
    device::{
        CreateDeviceImplError, CreateRenderPassError, Device, DeviceTrait,
    },
    fence::Fence,
    format::Format,
    image::{
        Image, ImageExtent, ImageInfo, ImageUsage, ImageView, ImageViewInfo,
        Samples,
    },
    physical::{DeviceInfo, Feature, PhysicalDevice, PhysicalDeviceTrait},
    pipeline::{
        GraphicsPipeline, GraphicsPipelineInfo, PipelineLayout,
        PipelineLayoutInfo,
    },
    queue::{
        CreateEncoderError, Family, FamilyInfo, Queue, QueueCapabilityFlags,
        QueueTrait,
    },
    render_pass::{Framebuffer, FramebufferInfo, RenderPass, RenderPassInfo},
    semaphore::{Semaphore, SemaphoreInfo},
    shader::{CreateShaderModuleError, ShaderModule, ShaderModuleInfo},
    stage::PipelineStageFlags,
    surface::{
        PresentMode, Surface, SurfaceCapabilities, SurfaceError, Swapchain,
        SwapchainImage, SwapchainImageInfo, SwapchainTrait,
    },
    Extent2d, Graphics, GraphicsTrait, OutOfMemory,
};
use std::{convert::TryInto, sync::Arc};
use wasm_bindgen::JsCast;

#[derive(Debug)]

pub(super) struct WebGlSwapchain {
    uid: usize,
    gl: web_sys::WebGl2RenderingContext,
    images: Vec<SwapchainImage>,
}

impl WebGlSwapchain {
    pub(super) fn new(device: &WebGlDevice) -> Self {
        WebGlSwapchain {
            uid: device.uid,
            gl: device.gl.clone(),
            images: Vec::new(),
        }
    }
}

impl SwapchainTrait for WebGlSwapchain {
    fn configure(
        &mut self,
        image_usage: ImageUsage,
        format: Format,
        mode: PresentMode,
    ) -> Result<(), SurfaceError> {
        if image_usage != ImageUsage::COLOR_ATTACHMENT {
            return Err(SurfaceError::UsageNotSupported { usage: image_usage });
        }

        if format != Format::RGBA8Srgb {
            return Err(SurfaceError::FormatUnsupported { format });
        }

        if mode != PresentMode::Fifo {
            return Err(SurfaceError::PresentModeUnsupported { mode });
        }

        let renderbuffer =
            self.gl
                .create_renderbuffer()
                .ok_or(SurfaceError::OutOfMemory {
                    source: OutOfMemory,
                })?;

        self.gl.bind_renderbuffer(
            web_sys::WebGl2RenderingContext::RENDERBUFFER,
            Some(&renderbuffer),
        );

        let width = self.gl.drawing_buffer_width();

        let height = self.gl.drawing_buffer_height();

        self.gl.renderbuffer_storage(
            web_sys::WebGl2RenderingContext::RENDERBUFFER,
            web_sys::WebGl2RenderingContext::SRGB8_ALPHA8,
            width,
            height,
        );

        let framebuffer =
            self.gl
                .create_framebuffer()
                .ok_or(SurfaceError::OutOfMemory {
                    source: OutOfMemory,
                })?;

        self.gl.bind_framebuffer(
            web_sys::WebGl2RenderingContext::READ_FRAMEBUFFER,
            Some(&framebuffer),
        );

        self.gl.framebuffer_renderbuffer(
            web_sys::WebGl2RenderingContext::READ_FRAMEBUFFER,
            web_sys::WebGl2RenderingContext::COLOR_ATTACHMENT0,
            web_sys::WebGl2RenderingContext::RENDERBUFFER,
            Some(&renderbuffer),
        );

        let semaphore =
            Semaphore::make(WebGlSemaphore { owner: self.uid }, SemaphoreInfo);

        self.images = vec![SwapchainImage::make(
            WebGlSwapchainImage {
                renderbuffer: renderbuffer.clone(),
                framebuffer,
                width,
                height,
                owner: self.uid,
            },
            SwapchainImageInfo {
                image: Image::make(
                    WebGlImage::renderbuffer(renderbuffer, self.uid),
                    ImageInfo {
                        extent: ImageExtent::D2 {
                            width: width.max(0) as _,
                            height: height.max(0) as _,
                        },
                        format: Format::RGBA8Srgb,
                        levels: 1,
                        layers: 1,
                        samples: Samples::Samples1,
                        usage: ImageUsage::COLOR_ATTACHMENT,
                    },
                ),
                wait: semaphore.clone(),
                signal: semaphore.clone(),
            },
        )];

        Ok(())
    }

    fn acquire_image(
        &mut self,
    ) -> Result<Option<SwapchainImage>, SurfaceError> {
        Ok(Some(self.images[0].clone()))
    }
}

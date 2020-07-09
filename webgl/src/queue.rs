use crate::{device::WebGlDevice, handle::*, JsError, TypeError};
use illume::{
    command::{Command, CommandBuffer, CommandBufferTrait},
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
        PipelineLayoutInfo, PrimitiveTopology, Viewport,
    },
    queue::{
        CreateEncoderError, Family, FamilyInfo, Queue, QueueCapabilityFlags,
        QueueTrait,
    },
    render_pass::{
        AttachmentInfo, ClearValue, Framebuffer, FramebufferInfo, RenderPass,
        RenderPassInfo, Subpass,
    },
    semaphore::{Semaphore, SemaphoreInfo},
    shader::{CreateShaderModuleError, ShaderModule, ShaderModuleInfo},
    stage::PipelineStageFlags,
    surface::{
        PresentMode, Surface, SurfaceCapabilities, SurfaceError, Swapchain,
        SwapchainImage, SwapchainTrait,
    },
    Extent2d, Graphics, GraphicsTrait, OutOfMemory, Rect2d,
};
use smallvec::SmallVec;
use std::{convert::TryInto, ops::Range, sync::Arc};
use wasm_bindgen::{JsCast, JsValue};

#[derive(Debug)]
enum WebGlCommand {
    BeginRenderPass {
        pass: RenderPass,
        framebuffer: Framebuffer,
        clears: SmallVec<[ClearValue; 4]>,
    },

    BindGraphicsPipeline {
        pipeline: GraphicsPipeline,
    },

    SetViewport {
        viewport: Viewport,
    },

    SetScissor {
        scissor: Rect2d,
    },

    Draw {
        vertices: Range<u32>,
        instances: Range<u32>,
    },
}

#[derive(Debug)]

struct WebGlCommandBuffer {
    commands: Vec<WebGlCommand>,
}

unsafe impl CommandBufferTrait for WebGlCommandBuffer {
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }

    fn write(&mut self, commands: &[Command<'_>]) -> Result<(), OutOfMemory> {
        self.commands = commands
            .iter()
            .filter_map(|cmd| match cmd {
                Command::BeginRenderPass {
                    pass,
                    framebuffer,
                    clears,
                } => Some(WebGlCommand::BeginRenderPass {
                    pass: (*pass).clone(),
                    framebuffer: (*framebuffer).clone(),
                    clears: clears.iter().copied().collect(),
                }),
                Command::EndRenderPass => None,
                Command::BindGraphicsPipeline { pipeline } => {
                    Some(WebGlCommand::BindGraphicsPipeline {
                        pipeline: (*pipeline).clone(),
                    })
                }
                Command::SetScissor { scissor } => {
                    Some(WebGlCommand::SetScissor { scissor: *scissor })
                }
                Command::SetViewport { viewport } => {
                    Some(WebGlCommand::SetViewport {
                        viewport: *viewport,
                    })
                }
                Command::Draw {
                    vertices,
                    instances,
                } => Some(WebGlCommand::Draw {
                    vertices: vertices.clone(),
                    instances: instances.clone(),
                }),
            })
            .collect();

        Ok(())
    }
}

impl QueueTrait for WebGlDevice {
    fn create_command_buffer(
        &mut self,
    ) -> Result<CommandBuffer, CreateEncoderError> {
        Ok(CommandBuffer::new(Box::new(WebGlCommandBuffer {
            commands: Vec::new(),
        })))
    }

    fn submit(
        &mut self,
        wait: &[(PipelineStageFlags, &Semaphore)],
        buffer: CommandBuffer,
        _signal: &[&Semaphore],
        fence: Option<&Fence>,
    ) {
        struct State<'a> {
            clears: &'a [ClearValue],
            framebuffers: &'a [web_sys::WebGlFramebuffer],
            subpasses: &'a [Subpass],
            subass_clears: &'a [WebGlSubpassClears],
            next_subpass: usize,
            mode: u32,
        }

        let mut state = State {
            clears: &[],
            framebuffers: &[],
            subpasses: &[],
            subass_clears: &[],
            next_subpass: 0,
            mode: 0,
        };

        macro_rules! next_subpass {
            () => {
                tracing::trace!("Next subpass");

                let subpass = &state.subpasses[state.next_subpass];

                let framebuffer = &state.framebuffers[state.next_subpass];

                let clears = &state.subass_clears[state.next_subpass];

                self.gl.bind_framebuffer(
                    web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER,
                    Some(&framebuffer),
                );

                let draw_buffers = (0..subpass.colors.len())
                    .map(|i| {
                        web_sys::WebGl2RenderingContext::COLOR_ATTACHMENT0
                            + i as u32
                    })
                    .map(JsValue::from)
                    .collect::<js_sys::Array>();

                self.gl.draw_buffers(&draw_buffers);

                for color_clear in &clears.colors {
                    match state.clears[color_clear.clear] {
                        ClearValue::Color(r, g, b, a) => {
                            self.gl.clear_bufferfv_with_f32_array(
                                web_sys::WebGl2RenderingContext::COLOR,
                                color_clear.index.try_into().unwrap(),
                                &[r, g, b, a],
                            )
                        }
                        ClearValue::DepthStencil(_, _) => {
                            panic!("Expected color clear value")
                        }
                    }
                }

                if let Some(clear_index) = clears.depth {
                    match state.clears[clear_index] {
                        ClearValue::Color(_, _, _, _) => {
                            panic!("Expected depth clear value")
                        }
                        ClearValue::DepthStencil(d, s) => {
                            self.gl.clear_bufferfi(
                                web_sys::WebGl2RenderingContext::DEPTH,
                                0,
                                d,
                                s.try_into().unwrap(),
                            )
                        }
                    }
                }
            };
        }

        let buffer = buffer.downcast::<WebGlCommandBuffer>();

        for command in &buffer.commands {
            match command {
                WebGlCommand::BeginRenderPass {
                    pass,
                    framebuffer,
                    clears,
                } => {
                    state.subass_clears = &pass.webgl_ref(&*self).clears;

                    state.framebuffers = &framebuffer.webgl_ref(&*self).handles;

                    state.clears = clears;

                    state.subpasses = &pass.info().subpasses;

                    state.next_subpass = 0;

                    next_subpass!();
                }
                WebGlCommand::BindGraphicsPipeline { pipeline } => {
                    let program = &pipeline.webgl_ref(&*self).program;

                    self.gl.use_program(Some(program));

                    state.mode = match pipeline.info().primitive_topology {
                        PrimitiveTopology::PointList => {
                            web_sys::WebGl2RenderingContext::POINTS
                        }
                        PrimitiveTopology::LineList => {
                            web_sys::WebGl2RenderingContext::LINES
                        }
                        PrimitiveTopology::LineStrip => {
                            web_sys::WebGl2RenderingContext::LINE_STRIP
                        }
                        PrimitiveTopology::TriangleFan => {
                            web_sys::WebGl2RenderingContext::TRIANGLE_FAN
                        }
                        PrimitiveTopology::TriangleList => {
                            web_sys::WebGl2RenderingContext::TRIANGLES
                        }
                        PrimitiveTopology::TriangleStrip => {
                            web_sys::WebGl2RenderingContext::TRIANGLE_STRIP
                        }
                    };

                    // FIXME: Set whole state.
                }
                WebGlCommand::SetScissor { scissor } => self.gl.scissor(
                    scissor.offset.x,
                    scissor.offset.y,
                    scissor.extent.width.min(i32::max_value() as u32) as i32,
                    scissor.extent.height.min(i32::max_value() as u32) as i32,
                ),
                WebGlCommand::SetViewport { viewport } => self.gl.viewport(
                    viewport.x.offset.min(i32::max_value() as f32) as i32,
                    viewport.y.offset.min(i32::max_value() as f32) as i32,
                    viewport.x.size.min(i32::max_value() as f32) as i32,
                    viewport.y.size.min(i32::max_value() as f32) as i32,
                ),
                WebGlCommand::Draw {
                    vertices,
                    instances,
                } => {
                    if instances.start != 0 {
                        tracing::error!(
                            "Non-zero base instance is unsupported yet"
                        );

                        continue;
                    }

                    if vertices.start > i32::max_value() as u32
                        || vertices.end > i32::max_value() as u32
                    {
                        tracing::error!(
                            "Vertex range {}..{} doesn't fit into i32",
                            vertices.start,
                            vertices.end
                        );

                        continue;
                    }

                    if instances.end > i32::max_value() as u32 {
                        tracing::error!(
                            "Instances range {}..{} doesn't fit into i32",
                            instances.start,
                            instances.end
                        );

                        continue;
                    }

                    self.gl.draw_arrays_instanced(
                        state.mode,
                        vertices.start as _,
                        (vertices.end - vertices.start) as _,
                        (instances.end - instances.start) as _,
                    );
                }
            }
        }

        if let Some(fence) = fence {
            let fence = fence.webgl_ref(&*self);

            let mut sync = fence.sync.borrow_mut();

            match &mut *sync {
                FenceState::Signalled => {
                    panic!("Fence is already singnalled state. Must be unsignalled")
                }
                FenceState::Pending(_) => {
                    panic!("Fence is already in pending singnalling state. Must be unsignalled")
                }
                FenceState::Unsignalled => {
                    *sync = FenceState::Pending(
                        self.gl
                            .fence_sync(
                                web_sys::WebGl2RenderingContext::SYNC_GPU_COMMANDS_COMPLETE,
                                0,
                            )
                            .unwrap(),
                    );
                }
            }
        }
    }

    fn present(&mut self, image: SwapchainImage) {
        let swapchain_image = image.webgl_ref(&*self);

        self.gl.bind_framebuffer(
            web_sys::WebGl2RenderingContext::READ_FRAMEBUFFER,
            Some(&swapchain_image.framebuffer),
        );

        let width = self.gl.drawing_buffer_width();

        let height = self.gl.drawing_buffer_height();

        self.gl.bind_framebuffer(
            web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER,
            None,
        );

        self.gl.blit_framebuffer(
            0,
            0,
            swapchain_image.width,
            swapchain_image.height,
            0,
            0,
            width,
            height,
            web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT,
            web_sys::WebGl2RenderingContext::NEAREST,
        );
    }
}

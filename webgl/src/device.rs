use crate::{handle::*, image::*, swapchain::WebGlSwapchain, JsError};
use illume::{
    AccelerationStructure, AccelerationStructureInfo, AspectFlags,
    AttachmentLoadOp, Buffer, BufferInfo, BufferUsage, CreateImageError,
    CreateRenderPassError, CreateShaderModuleError, DeviceTrait, Fence,
    FenceInfo, Format, Framebuffer, FramebufferInfo, GraphicsPipeline,
    GraphicsPipelineInfo, Image, ImageExtent, ImageInfo, ImageSubresource,
    ImageUsage, ImageView, ImageViewInfo, ImageViewKind, MemoryUsageFlags,
    OutOfMemory, PipelineLayout, PipelineLayoutInfo, RenderPass,
    RenderPassInfo, Samples, Semaphore, SemaphoreInfo, ShaderLanguage,
    ShaderModule, ShaderModuleInfo, Surface, SurfaceError, Swapchain,
};
use std::{
    cell::{Cell, Ref, RefCell},
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub(super) enum ExtensionState {
    Unknown,
    Missing,
    Init(js_sys::Object),
}

impl ExtensionState {
    fn unwrap_ref(&self) -> &js_sys::Object {
        match self {
            Self::Init(ext) => ext,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct WebGlDevice {
    pub(super) uid: usize,
    pub(super) gl: web_sys::WebGl2RenderingContext,
    pub(super) webgl_depth_texture: RefCell<ExtensionState>,
    pub(super) ext_color_buffer_float: RefCell<ExtensionState>,
}

impl PartialEq for WebGlDevice {
    fn eq(&self, rhs: &Self) -> bool {
        self.uid == rhs.uid
    }
}

impl Eq for WebGlDevice {}

impl WebGlDevice {
    pub(super) fn is(&self, uid: usize) -> bool {
        self.uid == uid
    }

    pub(super) fn new(gl: web_sys::WebGl2RenderingContext) -> Self {
        static UID_COUNTER: Cell<u64> = Cell::new(0);

        let mut uid = UID_COUNTER.load(Ordering::Relaxed);

        loop {
            if uid == !0 {
                panic!("Too many contexts created");
            }

            match UID_COUNTER.compare_exchange_weak(
                uid,
                uid + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new) => uid = new,
            }
        }

        WebGlDevice {
            uid,
            gl,
            webgl_depth_texture: RefCell::new(ExtensionState::Unknown),
            ext_color_buffer_float: RefCell::new(ExtensionState::Unknown),
        }
    }

    fn get_extension_in_place<'a>(
        &self,
        name: &'static str,
        ext: &'a RefCell<ExtensionState>,
    ) -> Option<Ref<'a, js_sys::Object>> {
        match &mut *ext.borrow_mut() {
            ExtensionState::Init(_) => {
                Some(Ref::map(ext.borrow(), ExtensionState::unwrap_ref))
            }
            ExtensionState::Missing => None,
            slot => {
                let extension = self.gl.get_extension(name).ok().flatten()?;

                *slot = ExtensionState::Init(extension);

                Some(Ref::map(ext.borrow(), ExtensionState::unwrap_ref))
            }
        }
    }

    pub(super) fn get_extension(
        &self,
        name: &'static str,
    ) -> Option<Ref<'_, js_sys::Object>> {
        match name {
            "WEBGL_depth_texture" => {
                self.get_extension_in_place(name, &self.webgl_depth_texture)
            }
            "EXT_color_buffer_float" => {
                self.get_extension_in_place(name, &self.ext_color_buffer_float)
            }
            _ => None,
        }
    }

    pub(super) fn has_extension(&self, name: &'static str) -> bool {
        self.get_extension(name).is_some()
    }
}

impl DeviceTrait for WebGlDevice {
    fn create_buffer(
        self: Arc<Self>,
        info: BufferInfo,
    ) -> Result<Buffer, OutOfMemory> {
        const TARGET: u32 = web_sys::WebGl2RenderingContext::ARRAY_BUFFER;

        if info.size as f64 as u64 != info.size {
            // FIXME: Workaround for sizes > 4.5 PiB
            return Err(OutOfMemory);
        }

        let buffer = self.gl.create_buffer().ok_or(OutOfMemory)?;

        self.gl.bind_buffer(TARGET, Some(&buffer));

        self.gl.buffer_data_with_f64(
            TARGET,
            info.size as f64,
            match info.memory {
                MemoryUsageFlags::Device => {
                    web_sys::WebGl2RenderingContext::DYNAMIC_COPY
                }
                MemoryUsageFlags::Dynamic => {
                    web_sys::WebGl2RenderingContext::DYNAMIC_DRAW
                }
                MemoryUsageFlags::Upload => {
                    web_sys::WebGl2RenderingContext::STATIC_DRAW
                }
                MemoryUsageFlags::Download => {
                    web_sys::WebGl2RenderingContext::STATIC_READ
                }
            },
        );

        self.gl.bind_buffer(TARGET, None);

        Ok(Buffer::make(
            WebGlBuffer {
                handle: buffer,
                owner: self.uid,
            },
            info,
        ))
    }

    fn create_buffer_static(
        self: Arc<Self>,
        info: BufferInfo,
        data: &[u8],
    ) -> Result<Buffer, OutOfMemory> {
        const TARGET: u32 = web_sys::WebGl2RenderingContext::ARRAY_BUFFER;

        if data.len() as f64 as usize != data.len() {
            // FIXME: Workaround for sizes > 4.5 PiB
            return Err(OutOfMemory);
        }

        let buffer = self.gl.create_buffer().ok_or(OutOfMemory)?;

        self.gl.bind_buffer(TARGET, Some(&buffer));

        self.gl.buffer_data_with_u8_array(
            TARGET,
            data,
            web_sys::WebGl2RenderingContext::STATIC_DRAW,
        );

        self.gl.bind_buffer(TARGET, None);

        Ok(Buffer::make(
            WebGlBuffer {
                handle: buffer,
                owner: self.uid,
            },
            info,
        ))
    }

    fn create_fence(self: Arc<Self>) -> Result<Fence, OutOfMemory> {
        Ok(Fence::make(
            WebGlFence {
                sync: RefCell::new(FenceState::Unsignalled),
                owner: self.uid,
            },
            FenceInfo,
        ))
    }

    fn create_framebuffer(
        self: Arc<Self>,
        info: FramebufferInfo,
    ) -> Result<Framebuffer, OutOfMemory> {
        let framebuffers = info
            .render_pass
            .info()
            .subpasses
            .iter()
            .map(|subpass| {
                let attach_attachment =
                    |view: usize, attachment: u32, expect_aspect: AspectFlags| {
                        let view = &info.views[view];
                        assert_eq!(view.info().view_kind, ImageViewKind::D2);

                        let ImageSubresource {
                            aspect,
                            first_level,
                            level_count,
                            first_layer,
                            layer_count,
                        } = view.info().subresource;

                        assert_eq!(aspect, expect_aspect);
                        assert_eq!(level_count, 1);
                        assert_eq!(layer_count, 1);

                        match &view.info().image.webgl_ref(&*self) {
                            WebGlImage::Texture { handle, .. } => {
                                self.gl.framebuffer_texture_layer(
                                    web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER,
                                    attachment,
                                    Some(handle),
                                    first_level.try_into().expect("Level index out of bounds"),
                                    first_layer.try_into().expect("Layer index out of bounds"),
                                )
                            }
                            WebGlImage::Renderbuffer { handle, .. } => {
                                assert_eq!(first_level, 0);
                                assert_eq!(first_layer, 0);
                                self.gl.framebuffer_renderbuffer(
                                    web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER,
                                    attachment,
                                    web_sys::WebGl2RenderingContext::RENDERBUFFER,
                                    Some(handle),
                                )
                            }
                        }
                    };

                let framebuffer = self.gl.create_framebuffer().ok_or(OutOfMemory)?;
                self.gl.bind_framebuffer(
                    web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER,
                    Some(&framebuffer),
                );

                for (index, color) in subpass.colors.iter().enumerate() {
                    attach_attachment(
                        *color,
                        web_sys::WebGl2RenderingContext::COLOR_ATTACHMENT0
                            + u32::try_from(index).expect("Color attachment index out of bound"),
                        AspectFlags::COLOR,
                    );
                }

                if let Some(depth) = subpass.depth {
                    attach_attachment(
                        depth,
                        web_sys::WebGl2RenderingContext::DEPTH_ATTACHMENT,
                        AspectFlags::DEPTH,
                    );
                }

                match self
                    .gl
                    .check_framebuffer_status(web_sys::WebGl2RenderingContext::DRAW_FRAMEBUFFER)
                {
                    web_sys::WebGl2RenderingContext::FRAMEBUFFER_COMPLETE => {
                        tracing::trace!("Framebuffer complete");
                    }
                    web_sys::WebGl2RenderingContext::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => {
                        tracing::error!("Framebuffer incomplete attachment");
                        panic!()
                    }
                    web_sys::WebGl2RenderingContext::FRAMEBUFFER_INCOMPLETE_DIMENSIONS => {
                        tracing::error!("Framebuffer incomplete dimensions");
                        panic!()
                    }
                    web_sys::WebGl2RenderingContext::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => {
                        tracing::error!("Framebuffer incomplete missing attachment");
                        panic!()
                    }
                    web_sys::WebGl2RenderingContext::FRAMEBUFFER_UNSUPPORTED => {
                        tracing::error!("Framebuffer unsupported");
                        panic!()
                    }
                    _ => {}
                }

                Ok(framebuffer)
            })
            .collect::<Result<Vec<_>, _>>()?;

        tracing::info!("Framebuffer created");

        Ok(Framebuffer::make(
            WebGlFramebuffer {
                handles: framebuffers,
                owner: self.uid,
            },
            info,
        ))
    }

    fn create_graphics_pipelines(
        self: Arc<Self>,
        infos: Vec<GraphicsPipelineInfo>,
    ) -> Result<Vec<GraphicsPipeline>, OutOfMemory> {
        infos
            .into_iter()
            .map(|info| -> Result<GraphicsPipeline, OutOfMemory> {
                assert_eq!(info.vertex_shader.entry, "main");

                let rasterizer = info
                    .rasterizer
                    .as_ref()
                    .expect("WebGL requires rasterizer");

                let fragment_shader_info = rasterizer
                    .fragment_shader
                    .as_ref()
                    .expect("WebGL requires fragment shader");

                let vertex_shader_source = std::str::from_utf8(
                    &info.vertex_shader.module.info().source,
                )
                .expect("GLSL code");

                let fragment_shader_source = std::str::from_utf8(
                    &fragment_shader_info.module.info().source,
                )
                .expect("GLSL code");

                let vertex_shader = self
                    .gl
                    .create_shader(
                        web_sys::WebGl2RenderingContext::VERTEX_SHADER,
                    )
                    .ok_or(OutOfMemory)?;

                self.gl.shader_source(&vertex_shader, vertex_shader_source);

                self.gl.compile_shader(&vertex_shader);

                let vertex_shader_info_log = self
                    .gl
                    .get_shader_info_log(&vertex_shader)
                    .ok_or(OutOfMemory)?;

                if vertex_shader_info_log.is_empty() {
                    tracing::trace!("Vertex shader compiled");
                } else {
                    tracing::error!(
                        "Vertex shader: {}",
                        vertex_shader_info_log
                    );
                }

                let fragment_shader = self
                    .gl
                    .create_shader(
                        web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
                    )
                    .ok_or(OutOfMemory)?;

                self.gl
                    .shader_source(&fragment_shader, fragment_shader_source);

                self.gl.compile_shader(&fragment_shader);

                let fragment_shader_info_log = self
                    .gl
                    .get_shader_info_log(&fragment_shader)
                    .ok_or(OutOfMemory)?;

                if fragment_shader_info_log.is_empty() {
                    tracing::trace!("Fragment shader compiled");
                } else {
                    tracing::error!(
                        "Fragment shader: {}",
                        fragment_shader_info_log
                    );
                }

                let program = self.gl.create_program().ok_or(OutOfMemory)?;

                self.gl.attach_shader(&program, &vertex_shader);

                self.gl.attach_shader(&program, &fragment_shader);

                self.gl.link_program(&program);

                self.gl.validate_program(&program);

                let program_info_log = self
                    .gl
                    .get_program_info_log(&program)
                    .ok_or(OutOfMemory)?;

                if program_info_log.is_empty() {
                    tracing::trace!("Program linked");
                } else {
                    tracing::error!("Program: {}", program_info_log);
                }

                tracing::trace!("Graphics pipeline created");

                Ok(GraphicsPipeline::make(
                    WebGlGraphicsPipeline {
                        program,
                        owner: self.uid,
                    },
                    info,
                ))
            })
            .collect()
    }

    fn create_image(
        self: Arc<Self>,
        info: ImageInfo,
    ) -> Result<Image, CreateImageError> {
        type GL = web_sys::WebGl2RenderingContext;

        let (webgl_info, webgl_kind) = webgl_image_info(&self, &info, false)
            .ok_or_else(|| CreateImageError::Unsupported { info })?;

        match webgl_kind {
            WebGlImageKind::Renderbuffer => {
                let renderbuffer =
                    self.gl.create_renderbuffer().ok_or(OutOfMemory)?;

                self.gl
                    .bind_renderbuffer(GL::RENDERBUFFER, Some(&renderbuffer));

                match info.extent {
                    ImageExtent::D1 { width } => {
                        self.gl.renderbuffer_storage(
                            GL::RENDERBUFFER,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            1,
                        );
                    }
                    ImageExtent::D2 { width, height } => {
                        self.gl.renderbuffer_storage(
                            GL::RENDERBUFFER,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                        );
                    }
                    _ => unreachable!("Renderbuffer kind should not be suggested for 3d images"),
                }

                Ok(Image::make(
                    WebGlImage::renderbuffer(renderbuffer, self.uid),
                    info,
                ))
            }
            WebGlImageKind::Texture => {
                let texture = self.gl.create_texture().ok_or(OutOfMemory)?;

                match info.extent {
                    ImageExtent::D1 { width } => {
                        self.gl.bind_texture(GL::TEXTURE_2D, Some(&texture));

                        self.gl.tex_storage_2d(
                            GL::TEXTURE_2D,
                            info.levels.try_into().map_err(|_| OutOfMemory)?,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            1,
                        );

                        self.gl.bind_texture(GL::TEXTURE_2D, None);
                    }
                    ImageExtent::D2 { width, height } if info.layers == 1 => {
                        self.gl.bind_texture(GL::TEXTURE_2D, Some(&texture));

                        self.gl.tex_storage_2d(
                            GL::TEXTURE_2D,
                            info.levels.try_into().map_err(|_| OutOfMemory)?,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                        );

                        self.gl.bind_texture(GL::TEXTURE_2D, None);
                    }
                    ImageExtent::D2 { width, height } => {
                        self.gl
                            .bind_texture(GL::TEXTURE_2D_ARRAY, Some(&texture));

                        self.gl.tex_storage_3d(
                            GL::TEXTURE_2D_ARRAY,
                            info.levels.try_into().map_err(|_| OutOfMemory)?,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                            info.layers.try_into().map_err(|_| OutOfMemory)?,
                        );

                        self.gl.bind_texture(GL::TEXTURE_2D_ARRAY, None);
                    }
                    ImageExtent::D3 {
                        width,
                        height,
                        depth,
                    } => {
                        self.gl.bind_texture(GL::TEXTURE_3D, Some(&texture));

                        self.gl.tex_storage_3d(
                            GL::TEXTURE_3D,
                            info.levels.try_into().map_err(|_| OutOfMemory)?,
                            webgl_info.internal,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                            depth.try_into().map_err(|_| OutOfMemory)?,
                        );

                        self.gl.bind_texture(GL::TEXTURE_3D, None);
                    }
                }

                Ok(Image::make(
                    WebGlImage::texture(texture, webgl_info, self.uid),
                    info,
                ))
            }
        }
    }

    fn create_image_static(
        self: Arc<Self>,
        info: ImageInfo,
        data: &[u8],
    ) -> Result<Image, CreateImageError> {
        type GL = web_sys::WebGl2RenderingContext;

        let js_error = |err| -> CreateImageError {
            CreateImageError::Other {
                source: Box::new(JsError(err)),
            }
        };

        let (webgl_info, webgl_kind) = webgl_image_info(&self, &info, true)
            .ok_or_else(|| CreateImageError::Unsupported { info })?;

        match webgl_kind {
            WebGlImageKind::Renderbuffer => {
                unreachable!("Shouldn't be returned because of `texture_only = true` supplied for `webgl_image_info` call")
            }
            WebGlImageKind::Texture => {
                let texture = self.gl.create_texture().ok_or(OutOfMemory)?;

                match info.extent {
                    ImageExtent::D1 { width } => {
                        self.gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
                        self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                            GL::TEXTURE_2D,
                            0,
                            webgl_info.internal as _,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            1,
                            0,
                            webgl_info.format,
                            webgl_info.repr,
                            Some(data),
                        ).map_err(js_error)?;
                        if info.levels > 1 {
                            self.gl.generate_mipmap(GL::TEXTURE_2D);
                        }
                        self.gl.bind_texture(GL::TEXTURE_2D, None);
                    }
                    ImageExtent::D2 { width, height } if info.layers == 1 => {
                        self.gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
                        self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                            GL::TEXTURE_2D,
                            0,
                            webgl_info.internal as _,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                            0,
                            webgl_info.format,
                            webgl_info.repr,
                            Some(data),
                        ).map_err(js_error)?;
                        if info.levels > 1 {
                            self.gl.generate_mipmap(GL::TEXTURE_2D);
                        }
                        self.gl.bind_texture(GL::TEXTURE_2D, None);
                    }
                    ImageExtent::D2 {
                        width,
                        height,
                    } => {
                        self.gl.bind_texture(GL::TEXTURE_2D_ARRAY, Some(&texture));
                        self.gl.tex_image_3d_with_opt_u8_array(
                            GL::TEXTURE_2D_ARRAY,
                            0,
                            webgl_info.internal as _,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                            info.layers.try_into().map_err(|_| OutOfMemory)?,
                            0,
                            webgl_info.format,
                            webgl_info.repr,
                            Some(data),
                        ).map_err(js_error)?;
                        if info.levels > 1 {
                            self.gl.generate_mipmap(GL::TEXTURE_2D_ARRAY);
                        }
                        self.gl.bind_texture(GL::TEXTURE_2D_ARRAY, None);
                    }
                    ImageExtent::D3 {
                        width,
                        height,
                        depth,
                    } => {
                        self.gl.bind_texture(GL::TEXTURE_3D, Some(&texture));
                        self.gl.tex_image_3d_with_opt_u8_array(
                            GL::TEXTURE_3D,
                            0,
                            webgl_info.internal as _,
                            width.try_into().map_err(|_| OutOfMemory)?,
                            height.try_into().map_err(|_| OutOfMemory)?,
                            depth.try_into().map_err(|_| OutOfMemory)?,
                            0,
                            webgl_info.format,
                            webgl_info.repr,
                            Some(data),
                        ).map_err(js_error)?;
                        if info.levels > 1 {
                            self.gl.generate_mipmap(GL::TEXTURE_3D);
                        }
                        self.gl.bind_texture(GL::TEXTURE_3D, None);
                    }
                }

                Ok(Image::make(
                    WebGlImage::texture(texture, webgl_info, self.uid),
                    info,
                ))
            }
        }
    }

    fn create_image_view(
        self: Arc<Self>,
        info: ImageViewInfo,
    ) -> Result<ImageView, OutOfMemory> {
        assert!(info.image.is_owner(&*self));

        Ok(ImageView::make(WebGlImageView, info))
    }

    fn create_pipeline_layout(
        self: Arc<Self>,
        info: PipelineLayoutInfo,
    ) -> Result<PipelineLayout, OutOfMemory> {
        Ok(PipelineLayout::make(
            WebGlPipelineLayout { owner: self.uid },
            info,
        ))
    }

    fn create_render_pass(
        self: Arc<Self>,
        render_pass_info: RenderPassInfo,
    ) -> Result<RenderPass, CreateRenderPassError> {
        let mut visited = HashSet::new();

        let mut last_clear_index = 0;

        let clear_indices = render_pass_info
            .attachments
            .iter()
            .enumerate()
            .filter_map(|(a, info)| {
                if info.load_op == AttachmentLoadOp::Clear {
                    last_clear_index += 1;

                    Some((a, last_clear_index - 1))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        let clears = render_pass_info
            .subpasses
            .iter()
            .map(|subpass| WebGlSubpassClears {
                colors: subpass
                    .colors
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &a)| {
                        if visited.insert(a) {
                            Some(WebGlColorClear {
                                index,
                                clear: clear_indices[&a],
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
                depth: subpass.depth.and_then(|a| {
                    if visited.insert(a) {
                        Some(clear_indices[&a])
                    } else {
                        None
                    }
                }),
            })
            .collect::<Vec<_>>();

        Ok(RenderPass::make(
            WebGlRenderPass {
                clears,
                owner: self.uid,
            },
            render_pass_info,
        ))
    }

    fn create_semaphore(self: Arc<Self>) -> Result<Semaphore, OutOfMemory> {
        Ok(Semaphore::make(
            WebGlSemaphore { owner: self.uid },
            SemaphoreInfo,
        ))
    }

    fn create_shader_module(
        self: Arc<Self>,
        info: ShaderModuleInfo,
    ) -> Result<ShaderModule, CreateShaderModuleError> {
        match info.language {
            ShaderLanguage::GLSL => Ok(ShaderModule::make(
                WebGlShaderModule { owner: self.uid },
                info,
            )),
            _ => Err(CreateShaderModuleError::UnsupportedShaderLanguage {
                language: info.language,
            }),
        }
    }

    fn create_swapchain(
        self: Arc<Self>,
        surface: &mut Surface,
    ) -> Result<Swapchain, SurfaceError> {
        if !surface.is_owner(&*self) {
            Err(SurfaceError::NotSupported)
        } else {
            Ok(Swapchain::new(Box::new(WebGlSwapchain::new(&*self))))
        }
    }

    fn reset_fences(&self, fences: &[&Fence]) {
        for fence in fences {
            let mut sync = fence.webgl_ref(self).sync.borrow_mut();

            match &*sync {
                FenceState::Pending(_) => panic!("Cannot reset pending fence"),
                FenceState::Signalled => *sync = FenceState::Unsignalled,
                FenceState::Unsignalled => {}
            }
        }
    }

    fn wait_fences(&self, fences: &[&Fence], all: bool) {
        if !all {
            // Waiting for at least one fence.
            match fences.iter().try_fold(false, |acc, fence| {
                // Borrow next fence.
                let mut borrow = fence.webgl_ref(self).sync.borrow_mut();
                match &mut *borrow {
                    FenceState::Pending(sync) => {
                        // Wait for fence with 0 timeout to check status.
                        match self.gl.client_wait_sync_with_u32(
                            sync,
                            web_sys::WebGl2RenderingContext::SYNC_FLUSH_COMMANDS_BIT,
                            0,
                        ) {
                            web_sys::WebGl2RenderingContext::ALREADY_SIGNALED
                            | web_sys::WebGl2RenderingContext::CONDITION_SATISFIED => {
                                // Signalled. Remove sync object and stop iteration.
                                *borrow = FenceState::Signalled;
                                Err(())
                            }
                            web_sys::WebGl2RenderingContext::TIMEOUT_EXPIRED => {
                                // Can't wait on client.
                                // Wait on server and continue iteration.
                                self.gl.wait_sync_with_f64(
                                    sync,
                                    0,
                                    web_sys::WebGl2RenderingContext::TIMEOUT_IGNORED,
                                );
                                *borrow = FenceState::Signalled;
                                Ok(true)
                            }
                            _ => panic!("Unexpected result"),
                        }
                    }
                    FenceState::Signalled => Err(()), // One fence is already signalled - return immediatelly.
                    FenceState::Unsignalled => {
                        tracing::warn!("Waiting on unsignalled fence. It will never become signalled. Another fence may become signalled and unblock execution.");
                        // Continue iteration and look for pending or signalled fence.
                        Ok(acc)
                    }
                }
            }) {
                Err(()) => return, // At least one fence is signalled
                Ok(true) => return, // Server waits for at least one fence
                Ok(false) => panic!("All fences are unsignalled and will never become signalled. That makes infinite wait"),
            }
        } else {
            // Waiting for all fences.
            // Get maximum client wait timeout.
            let max_client_wait_timeout = self
                .gl
                .get_parameter(web_sys::WebGl2RenderingContext::MAX_CLIENT_WAIT_TIMEOUT_WEBGL)
                .unwrap()
                .as_f64()
                .unwrap();

            // Check timing to avoid waiting for too long.
            let performance = web_sys::window().unwrap().performance().unwrap();

            let start = performance.now();

            let mut client_wait_timeout = max_client_wait_timeout;

            for fence in fences {
                // Borrow next fence.
                let mut borrow = fence.webgl_ref(self).sync.borrow_mut();

                match &mut *borrow {
                    FenceState::Pending(sync) => {
                        // Wait for maximum duration.
                        match self.gl.client_wait_sync_with_f64(
                            sync,
                            web_sys::WebGl2RenderingContext::SYNC_FLUSH_COMMANDS_BIT,
                            client_wait_timeout,
                        ) {
                            web_sys::WebGl2RenderingContext::ALREADY_SIGNALED
                            | web_sys::WebGl2RenderingContext::CONDITION_SATISFIED => {
                                // Reduce wait duration.
                                client_wait_timeout =
                                    0f64.max(start + max_client_wait_timeout - performance.now());
                                // Signalled. Remove sync object and continue with other fences.
                                *borrow = FenceState::Signalled;
                            }
                            web_sys::WebGl2RenderingContext::TIMEOUT_EXPIRED => {
                                // Timeout. Zero duration and wait on server instead.
                                client_wait_timeout = 0f64;
                                self.gl.wait_sync_with_f64(
                                    sync,
                                    0,
                                    web_sys::WebGl2RenderingContext::TIMEOUT_IGNORED,
                                );
                                *borrow = FenceState::Signalled;
                            }
                            _ => panic!("Unexpected result"),
                        }
                    }
                    FenceState::Signalled => {
                        // Already signalled. Continue with other fences.
                    }
                    FenceState::Unsignalled => {
                        panic!("Fence is unsignalled and will never become signalled. That makes infinite wait")
                    }
                }
            }
        }
    }

    fn wait_idle(&self) {
        self.gl.finish();
    }

    fn create_acceleration_structure(
        self: Arc<Self>,
        _: AccelerationStructureInfo,
    ) -> AccelerationStructure {
        panic!("WebGL doesn't support RayTracing");
    }
}

enum ImageKind {
    Renderbuffer,
    Texture,
}

#[derive(Debug, thiserror::Error)]
#[error("Missing WEBGL_depth_texture extension")]

struct MissingWebGlDepthTextureExtension;

fn image_kind_for_usage_format(
    device: &WebGlDevice,
    usage: ImageUsage,
    format: Format,
) -> Result<ImageKind, MissingWebGlDepthTextureExtension> {
    if format.is_depth() || format.is_stencil() {
        assert!(
            !usage.contains(ImageUsage::COLOR_ATTACHMENT),
            "Images with depth-stencil format could not be used as color attachment"
        );

        if usage.intersects(
            ImageUsage::SAMPLED
                | ImageUsage::STORAGE
                | ImageUsage::INPUT_ATTACHMENT,
        ) {
            device
                .get_extension("WEBGL_depth_texture")
                .ok_or(MissingWebGlDepthTextureExtension)?;

            Ok(ImageKind::Texture)
        } else if usage.contains(ImageUsage::DEPTH_STENCIL_ATTACHMENT)
            || device.get_extension("WEBGL_depth_texture").is_none()
        {
            Ok(ImageKind::Renderbuffer)
        } else {
            Ok(ImageKind::Texture)
        }
    } else if format.is_color() {
        assert!(
            !usage.contains(ImageUsage::DEPTH_STENCIL_ATTACHMENT),
            "Images with color format could not be used as depth-stencil attachment"
        );

        if usage.intersects(
            ImageUsage::SAMPLED
                | ImageUsage::STORAGE
                | ImageUsage::INPUT_ATTACHMENT,
        ) {
            Ok(ImageKind::Texture)
        } else if usage.contains(ImageUsage::COLOR_ATTACHMENT) {
            Ok(ImageKind::Renderbuffer)
        } else {
            Ok(ImageKind::Texture)
        }
    } else {
        unreachable!("Format must be color or depth and/or stencil")
    }
}

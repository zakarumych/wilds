use crate::renderer::PositionNormalTangent3d;

use {
    super::Pass,
    crate::renderer::{
        vertex::{
            vertex_layouts_for_pipeline, PositionNormalTangent3dUV,
            VertexType as _,
        },
        Context,
    },
    bumpalo::{collections::Vec as BVec, Bump},
    color_eyre::Report,
    hecs::World,
    illume::*,
    smallvec::smallvec,
};

pub struct Input {
    target: Image,
}

pub struct Output;

pub struct RasterPass {
    render_pass: RenderPass,
    pipeline_layout: PipelineLayout,
    graphics_pipeline: GraphicsPipeline,
    framebuffers: lru::LruCache<Image, Framebuffer>,
}

impl RasterPass {
    pub fn new(ctx: &Context) -> Result<Self, Report> {
        let vert = VertexShader::new(
            ctx.create_shader_module(ShaderModuleInfo::spirv(
                include_bytes!("raster/main.vert.spv").to_vec(),
            ))?,
            "main",
        );

        let frag = FragmentShader::new(
            ctx.create_shader_module(ShaderModuleInfo::spirv(
                include_bytes!("raster/main.frag.spv").to_vec(),
            ))?,
            "main",
        );

        let render_pass = ctx.create_render_pass(RenderPassInfo {
            attachments: smallvec![
                AttachmentInfo {
                    format: Format::D32Sfloat,
                    samples: Samples::Samples1,
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::DontCare,
                    initial_layout: None,
                    final_layout: Layout::DepthStencilAttachmentOptimal,
                },
                AttachmentInfo {
                    format: Format::RGB8Unorm,
                    samples: Samples::Samples1,
                    load_op: AttachmentLoadOp::DontCare,
                    store_op: AttachmentStoreOp::Store,
                    initial_layout: None,
                    final_layout: Layout::Present,
                },
            ],
            subpasses: smallvec![Subpass {
                colors: smallvec![1],
                depth: Some(0),
            }],
            dependencies: smallvec![],
        })?;

        let pipeline_layout =
            ctx.create_pipeline_layout(PipelineLayoutInfo {
                sets: vec![],
                push_constants: vec![PushConstant {
                    stages: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: 64,
                }],
            })?;

        let (vertex_bindings, vertex_attributes) =
            vertex_layouts_for_pipeline(&[PositionNormalTangent3dUV::layout()]);

        let graphics_pipeline =
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_bindings: vertex_bindings,
                vertex_attributes: vertex_attributes,
                vertex_shader: vert,
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag,
                }
            })?;

        Ok(RasterPass {
            render_pass,
            pipeline_layout,
            graphics_pipeline,
            framebuffers: lru::LruCache::new(4),
        })
    }
}

impl Pass<'_> for RasterPass {
    type Input = Input;
    type Output = Output;

    fn draw(
        &mut self,
        input: Input,
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<Output, Report> {
        let target = input.target;

        let framebuffer;
        let fb = match self.framebuffers.get(&target) {
            Some(fb) => fb,
            None => {
                let extent = target.info().extent.into_2d();
                let view =
                    ctx.create_image_view(ImageViewInfo::new(target.clone()))?;
                framebuffer = ctx.create_framebuffer(FramebufferInfo {
                    render_pass: self.render_pass.clone(),
                    views: smallvec![view],
                    extent,
                })?;

                self.framebuffers.put(target, framebuffer.clone());
                &framebuffer
            }
        };

        let mut encoder = ctx.queue.create_encoder()?;

        let encoder = encoder.with_render_pass(
            &self.render_pass,
            fb,
            &[ClearValue::DepthStencil(1.0, 0)],
        );

        Ok(Output)
    }
}

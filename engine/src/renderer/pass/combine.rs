use {
    super::Pass,
    crate::renderer::Context,
    bumpalo::{collections::Vec as BVec, Bump},
    color_eyre::Report,
    hecs::World,
    illume::*,
    lru::LruCache,
    smallvec::smallvec,
};

pub struct Input {
    pub albedo: Image,
    pub normal_depth: Image,
    pub emissive: Image,
    pub direct: Image,
    pub diffuse: Image,
    pub combined: Image,
}

pub struct Output;

pub struct CombinePass {
    sampler: Sampler,
    albedo: [Option<ImageView>; 2],
    normal_depth: [Option<ImageView>; 2],
    emissive: [Option<ImageView>; 2],
    direct: [Option<ImageView>; 2],
    diffuse: [Option<ImageView>; 2],

    framebuffer: LruCache<Image, Framebuffer>,

    render_pass: Option<RenderPass>,
    pipeline: Option<GraphicsPipeline>,

    vert: VertexShader,
    frag: FragmentShader,

    pipeline_layout: PipelineLayout,
    per_frame_sets: [DescriptorSet; 2],
}

impl CombinePass {
    pub fn new(ctx: &mut Context) -> Result<Self, Report> {
        let set_layout =
            ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::UPDATE_AFTER_BIND_POOL,
                bindings: vec![
                    // Inputs
                    // Albedo
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // normal-depth
                    DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // emissive
                    DescriptorSetLayoutBinding {
                        binding: 2,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // direct
                    DescriptorSetLayoutBinding {
                        binding: 3,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // diffuse
                    DescriptorSetLayoutBinding {
                        binding: 4,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                ],
            })?;

        let pipeline_layout =
            ctx.create_pipeline_layout(PipelineLayoutInfo {
                sets: vec![set_layout.clone()],
                push_constants: vec![PushConstant {
                    stages: ShaderStageFlags::FRAGMENT,
                    offset: 0,
                    size: 8,
                }],
            })?;

        let vert = VertexShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("combine/combine.vert.spv").to_vec())
                    .into(),
            )?,
        );

        let frag = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("combine/combine.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let set0 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let set1 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let sampler = ctx.create_sampler(SamplerInfo {
            unnormalized_coordinates: false,
            min_lod: 0.0.into(),
            max_lod: 0.0.into(),
            address_mode_u: SamplerAddressMode::ClampToEdge,
            address_mode_v: SamplerAddressMode::ClampToEdge,
            address_mode_w: SamplerAddressMode::ClampToEdge,
            ..Default::default()
        })?;

        Ok(CombinePass {
            sampler,
            albedo: [None, None],
            normal_depth: [None, None],
            emissive: [None, None],
            direct: [None, None],
            diffuse: [None, None],

            framebuffer: LruCache::new(3),

            render_pass: None,
            pipeline: None,

            per_frame_sets: [set0, set1],
            pipeline_layout,

            vert,
            frag,
        })
    }
}

impl<'a> Pass<'a> for CombinePass {
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
        _world: &mut World,
        bump: &Bump,
    ) -> Result<Output, Report> {
        tracing::trace!("CombinePass::draw");
        let combined_info = input.combined.info();
        let extent = combined_info.extent.into_2d();
        let format = combined_info.format;

        let render_pass = match &self.render_pass {
            Some(render_pass)
                if render_pass.info().attachments[0].format == format =>
            {
                render_pass
            }
            _ => {
                self.framebuffer.clear();
                self.pipeline = None;
                self.render_pass = None;
                let render_pass = ctx.create_render_pass(RenderPassInfo {
                    attachments: smallvec![AttachmentInfo {
                        format,
                        samples: Samples::Samples1,
                        load_op: AttachmentLoadOp::Clear,
                        store_op: AttachmentStoreOp::Store,
                        initial_layout: None,
                        final_layout: Layout::Present,
                    }],
                    subpasses: smallvec![Subpass {
                        colors: smallvec![0],
                        depth: None,
                    }],
                    dependencies: smallvec![
                        SubpassDependency {
                            src: None,
                            dst: Some(0),
                            src_stages:
                                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            dst_stages:
                                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        },
                        SubpassDependency {
                            src: Some(0),
                            dst: None,
                            src_stages:
                                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            dst_stages:
                                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        },
                    ],
                })?;
                self.render_pass.get_or_insert(render_pass)
            }
        };

        let pipeline = match &self.pipeline {
            Some(pipeline) => pipeline,
            _ => {
                self.pipeline = None;

                let pipeline =
                    ctx.create_graphics_pipeline(graphics_pipeline_info! {
                        vertex_shader: self.vert.clone(),
                        layout: self.pipeline_layout.clone(),
                        render_pass: render_pass.clone(),
                        rasterizer: rasterizer!{
                            fragment_shader: self.frag.clone(),
                        }
                    })?;

                self.pipeline.get_or_insert(pipeline)
            }
        };

        let framebuffer = match self.framebuffer.get(&input.combined) {
            Some(framebuffer) => {
                assert_eq!(framebuffer.info().render_pass, *render_pass);
                framebuffer.clone()
            }
            None => {
                let combined = ctx.create_image_view(ImageViewInfo::new(
                    input.combined.clone(),
                ))?;

                let framebuffer = ctx.create_framebuffer(FramebufferInfo {
                    render_pass: render_pass.clone(),
                    views: smallvec![combined],
                    extent,
                })?;

                self.framebuffer
                    .put(input.combined.clone(), framebuffer.clone());

                framebuffer
            }
        };

        let mut writes = BVec::with_capacity_in(4, bump);

        let fid = (frame % 2) as u32;
        let set = &self.per_frame_sets[fid as usize];

        match &self.albedo[fid as usize] {
            Some(albedo) if albedo.info().image == input.albedo => {}
            _ => {
                self.albedo[fid as usize] = None;
                let albedo = ctx.create_image_view(ImageViewInfo::new(
                    input.albedo.clone(),
                ))?;
                let albedo = self.albedo[fid as usize].get_or_insert(albedo);
                writes.push(WriteDescriptorSet {
                    set,
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            albedo.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });
            }
        }

        match &self.normal_depth[fid as usize] {
            Some(normal_depth)
                if normal_depth.info().image == input.normal_depth => {}
            _ => {
                self.normal_depth[fid as usize] = None;
                let normal_depth = ctx.create_image_view(
                    ImageViewInfo::new(input.normal_depth.clone()),
                )?;
                let normal_depth =
                    self.normal_depth[fid as usize].get_or_insert(normal_depth);
                writes.push(WriteDescriptorSet {
                    set,
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            normal_depth.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });
            }
        }

        match &self.emissive[fid as usize] {
            Some(emissive) if emissive.info().image == input.emissive => {}
            _ => {
                self.emissive[fid as usize] = None;
                let emissive = ctx.create_image_view(ImageViewInfo::new(
                    input.emissive.clone(),
                ))?;
                let emissive =
                    self.emissive[fid as usize].get_or_insert(emissive);
                writes.push(WriteDescriptorSet {
                    set,
                    binding: 2,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            emissive.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });
            }
        }

        match &self.direct[fid as usize] {
            Some(direct) if direct.info().image == input.direct => {}
            _ => {
                self.direct[fid as usize] = None;
                let direct = ctx.create_image_view(ImageViewInfo::new(
                    input.direct.clone(),
                ))?;
                let direct = self.direct[fid as usize].get_or_insert(direct);
                writes.push(WriteDescriptorSet {
                    set,
                    binding: 3,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            direct.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });
            }
        }

        match &self.diffuse[fid as usize] {
            Some(diffuse) if diffuse.info().image == input.diffuse => {}
            _ => {
                self.diffuse[fid as usize] = None;
                let diffuse = ctx.create_image_view(ImageViewInfo::new(
                    input.diffuse.clone(),
                ))?;
                let diffuse = self.diffuse[fid as usize].get_or_insert(diffuse);
                writes.push(WriteDescriptorSet {
                    set,
                    binding: 4,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            diffuse.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });
            }
        }

        ctx.update_descriptor_sets(&writes, &[]);

        let mut encoder = ctx.queue.create_encoder()?;

        let mut render_pass_encoder = encoder.with_render_pass(
            render_pass,
            &framebuffer,
            &[ClearValue::Color(0.3, 0.4, 0.5, 1.0)],
        );

        render_pass_encoder.bind_graphics_pipeline(pipeline);
        render_pass_encoder.bind_graphics_descriptor_sets(
            &self.pipeline_layout,
            0,
            std::slice::from_ref(set),
            &[],
        );

        let extent_push_constant = [extent.width, extent.height];
        render_pass_encoder.push_constants(
            &self.pipeline_layout,
            ShaderStageFlags::FRAGMENT,
            0,
            &extent_push_constant,
        );
        render_pass_encoder.set_viewport(Viewport {
            x: Bounds {
                offset: 0.0.into(),
                size: (extent.width as f32).into(),
            },
            y: Bounds {
                offset: 0.0.into(),
                size: (extent.height as f32).into(),
            },
            z: Bounds {
                offset: 0.0.into(),
                size: 1.0.into(),
            },
        });

        render_pass_encoder.set_scissor(extent.into());
        render_pass_encoder.draw(0..3, 0..1);
        drop(render_pass_encoder);
        ctx.queue.submit(wait, encoder.finish(), signal, fence);

        Ok(Output)
    }
}

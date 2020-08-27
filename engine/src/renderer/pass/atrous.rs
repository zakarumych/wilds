use {
    super::Pass,
    crate::renderer::Context,
    bumpalo::{collections::Vec as BVec, Bump},
    color_eyre::Report,
    hecs::World,
    illume::*,
    smallvec::smallvec,
};

pub struct Input {
    pub normal_depth: Image,
    pub unfiltered: Image,
}

pub struct Output {
    pub filtered: Image,
}

pub struct ATrousFilter {
    sampler: Sampler,
    normal_depth: Option<ImageView>,
    unfiltered: Option<ImageView>,

    filtered: Option<[ImageView; 2]>,
    framebuffers: Option<[Framebuffer; 2]>,

    render_pass: RenderPass,
    pipelines: [GraphicsPipeline; 6],

    pipeline_layout: PipelineLayout,
    sets: [DescriptorSet; 3],
}

impl ATrousFilter {
    pub fn new(ctx: &mut Context) -> Result<Self, Report> {
        let set_layout =
            ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::UPDATE_AFTER_BIND_POOL,
                bindings: vec![
                    // Normal-Depth
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: DescriptorType::CombinedImageSampler,
                        count: 1,
                        stages: ShaderStageFlags::FRAGMENT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Unfiltered
                    DescriptorSetLayoutBinding {
                        binding: 1,
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
                push_constants: Vec::new(),
            })?;

        let vert = VertexShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous.vert.spv").to_vec())
                    .into(),
            )?,
        );

        let frag0h = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous0h.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let frag1h = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous1h.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let frag2h = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous2h.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let frag0v = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous0v.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let frag1v = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous1v.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let frag2v = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("atrous/atrous2v.frag.spv").to_vec())
                    .into(),
            )?,
        );

        let set0 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let set1 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let set2 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let sampler = ctx.create_sampler(SamplerInfo {
            unnormalized_coordinates: true,
            min_lod: 0.0.into(),
            max_lod: 0.0.into(),
            address_mode_u: SamplerAddressMode::ClampToEdge,
            address_mode_v: SamplerAddressMode::ClampToEdge,
            address_mode_w: SamplerAddressMode::ClampToEdge,
            ..Default::default()
        })?;

        let render_pass = ctx.create_render_pass(RenderPassInfo {
            attachments: smallvec![AttachmentInfo {
                format: Format::RGBA32Sfloat,
                samples: Samples::Samples1,
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                initial_layout: None,
                final_layout: Layout::ShaderReadOnlyOptimal,
            }],
            subpasses: smallvec![Subpass {
                colors: smallvec![0],
                depth: None,
            }],
            dependencies: smallvec![
                SubpassDependency {
                    src: None,
                    dst: Some(0),
                    src_stages: PipelineStageFlags::FRAGMENT_SHADER,
                    dst_stages: PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                },
                SubpassDependency {
                    src: Some(0),
                    dst: None,
                    src_stages: PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    dst_stages: PipelineStageFlags::FRAGMENT_SHADER,
                },
            ],
        })?;

        let pipelines = [
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert.clone(),
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag0h,
                }
            })?,
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert.clone(),
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag1h,
                }
            })?,
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert.clone(),
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag2h,
                }
            })?,
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert.clone(),
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag0v,
                }
            })?,
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert.clone(),
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag1v,
                }
            })?,
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert,
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag2v,
                }
            })?,
        ];

        Ok(ATrousFilter {
            sampler,
            normal_depth: None,
            unfiltered: None,
            filtered: None,
            framebuffers: None,

            sets: [set0, set1, set2],
            pipeline_layout,
            render_pass,
            pipelines,
        })
    }
}

impl<'a> Pass<'a> for ATrousFilter {
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
        let extent = input.normal_depth.info().extent.into_2d();

        let mut writes = BVec::with_capacity_in(4, bump);

        let filtered = match &self.filtered {
            Some(filtered)
                if filtered[0].info().image.info().extent.into_2d()
                    == extent =>
            {
                filtered
            }
            _ => {
                self.framebuffers = None;
                self.filtered = None;

                let filtered0 = ctx.create_image(ImageInfo {
                    extent: extent.into(),
                    format: Format::RGBA32Sfloat,
                    levels: 1,
                    layers: 1,
                    samples: Samples1,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
                    memory: MemoryUsageFlags::empty(),
                })?;

                let filtered1 = ctx.create_image(ImageInfo {
                    extent: extent.into(),
                    format: Format::RGBA32Sfloat,
                    levels: 1,
                    layers: 1,
                    samples: Samples1,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
                    memory: MemoryUsageFlags::empty(),
                })?;

                let filtered0 =
                    ctx.create_image_view(ImageViewInfo::new(filtered0))?;
                let filtered1 =
                    ctx.create_image_view(ImageViewInfo::new(filtered1))?;

                writes.push(WriteDescriptorSet {
                    set: &self.sets[1],
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            filtered0.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                writes.push(WriteDescriptorSet {
                    set: &self.sets[2],
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            filtered1.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                self.filtered.get_or_insert([filtered0, filtered1])
            }
        };

        let framebuffers = match &self.framebuffers {
            Some(framebuffers) => {
                assert_eq!(framebuffers[0].info().views[0], filtered[0]);
                assert_eq!(framebuffers[1].info().views[0], filtered[1]);
                framebuffers
            }
            _ => {
                self.framebuffers = None;
                let framebuffer0 = ctx.create_framebuffer(FramebufferInfo {
                    render_pass: self.render_pass.clone(),
                    views: smallvec![filtered[0].clone()],
                    extent,
                })?;
                let framebuffer1 = ctx.create_framebuffer(FramebufferInfo {
                    render_pass: self.render_pass.clone(),
                    views: smallvec![filtered[1].clone()],
                    extent,
                })?;
                self.framebuffers
                    .get_or_insert([framebuffer0, framebuffer1])
            }
        };

        match &self.normal_depth {
            Some(normal_depth)
                if normal_depth.info().image == input.normal_depth => {}
            _ => {
                self.normal_depth = None;
                let normal_depth = ctx.create_image_view(
                    ImageViewInfo::new(input.normal_depth.clone()),
                )?;

                writes.push(WriteDescriptorSet {
                    set: &self.sets[0],
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            normal_depth.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                writes.push(WriteDescriptorSet {
                    set: &self.sets[1],
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            normal_depth.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                writes.push(WriteDescriptorSet {
                    set: &self.sets[2],
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            normal_depth.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                self.normal_depth = Some(normal_depth);
            }
        };

        match &self.unfiltered {
            Some(unfiltered) if unfiltered.info().image == input.unfiltered => {
            }
            _ => {
                self.unfiltered = None;
                let unfiltered = ctx.create_image_view(ImageViewInfo::new(
                    input.unfiltered.clone(),
                ))?;

                writes.push(WriteDescriptorSet {
                    set: &self.sets[0],
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [(
                            unfiltered.clone(),
                            Layout::ShaderReadOnlyOptimal,
                            self.sampler.clone(),
                        )],
                    )),
                });

                self.unfiltered = Some(unfiltered);
            }
        };

        if !writes.is_empty() {
            ctx.update_descriptor_sets(&writes, &[]);
        }

        let mut encoder = ctx.queue.create_encoder()?;

        const SET_INDICES: [usize; 6] = [0, 1, 2, 1, 2, 1];

        for i in 0..6 {
            let mut render_pass_encoder = encoder.with_render_pass(
                &self.render_pass,
                &framebuffers[i % 2],
                &[ClearValue::Color(0.3, 0.4, 0.5, 1.0)],
            );

            render_pass_encoder.bind_graphics_pipeline(&self.pipelines[i]);
            render_pass_encoder.bind_graphics_descriptor_sets(
                &self.pipeline_layout,
                0,
                std::slice::from_ref(&self.sets[SET_INDICES[i]]),
                &[],
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
        }

        ctx.queue.submit(wait, encoder.finish(), signal, fence);

        Ok(Output {
            filtered: filtered[1].info().image.clone(),
        })
    }
}

use {
    super::Pass, crate::renderer::Context, bumpalo::Bump, color_eyre::Report,
    hecs::World, illume::*, smallvec::smallvec,
};

pub struct Input {
    pub normal_depth: Image,
    pub unfiltered: Image,
}

pub struct Output {
    pub filtered: Image,
}

pub struct GaussFilter {
    sampler: Sampler,
    normal_depth: [Option<ImageView>; 2],
    unfiltered: [Option<ImageView>; 2],
    // intermediate: Option<ImageView>,
    filtered: Option<ImageView>,
    framebuffer: Option<Framebuffer>,

    render_pass: RenderPass,
    pipeline: GraphicsPipeline,

    pipeline_layout: PipelineLayout,
    per_frame_sets: [DescriptorSet; 2],
}

impl GaussFilter {
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
                Spirv::new(
                    include_bytes!("gauss_filter/gauss_filter.vert.spv")
                        .to_vec(),
                )
                .into(),
            )?,
        );

        let frag = FragmentShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("gauss_filter/gauss_filter.frag.spv")
                        .to_vec(),
                )
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

        let pipeline =
            ctx.create_graphics_pipeline(graphics_pipeline_info! {
                vertex_shader: vert,
                layout: pipeline_layout.clone(),
                render_pass: render_pass.clone(),
                rasterizer: rasterizer!{
                    fragment_shader: frag,
                }
            })?;

        Ok(GaussFilter {
            sampler,
            normal_depth: [None, None],
            unfiltered: [None, None],
            filtered: None,
            framebuffer: None,

            per_frame_sets: [set0, set1],
            pipeline_layout,
            render_pass,
            pipeline,
        })
    }
}

impl<'a> Pass<'a> for GaussFilter {
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

        let filtered = match &self.filtered {
            Some(filtered)
                if filtered.info().image.info().extent.into_2d() == extent =>
            {
                filtered
            }
            _ => {
                self.framebuffer = None;
                self.filtered = None;
                let filtered = ctx.create_image(ImageInfo {
                    extent: extent.into(),
                    format: Format::RGBA32Sfloat,
                    levels: 1,
                    layers: 1,
                    samples: Samples1,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
                })?;

                let filtered =
                    ctx.create_image_view(ImageViewInfo::new(filtered))?;
                self.filtered.get_or_insert(filtered)
            }
        };

        let framebuffer = match &self.framebuffer {
            Some(framebuffer) => {
                assert_eq!(framebuffer.info().views[0], *filtered);
                framebuffer
            }
            _ => {
                self.framebuffer = None;
                let framebuffer = ctx.create_framebuffer(FramebufferInfo {
                    render_pass: self.render_pass.clone(),
                    views: smallvec![filtered.clone()],
                    extent,
                })?;
                self.framebuffer.get_or_insert(framebuffer)
            }
        };

        let fid = (frame % 2) as u32;
        let set = &self.per_frame_sets[fid as usize];

        let mut update_set = false;
        let normal_depth = match &self.normal_depth[fid as usize] {
            Some(normal_depth)
                if normal_depth.info().image == input.normal_depth =>
            {
                normal_depth
            }
            _ => {
                update_set = true;
                self.normal_depth[fid as usize] = None;
                let normal_depth = ctx.create_image_view(
                    ImageViewInfo::new(input.normal_depth.clone()),
                )?;
                self.normal_depth[fid as usize].get_or_insert(normal_depth)
            }
        };

        let unfiltered = match &self.unfiltered[fid as usize] {
            Some(unfiltered) if unfiltered.info().image == input.unfiltered => {
                unfiltered
            }
            _ => {
                update_set = true;
                self.unfiltered[fid as usize] = None;
                let unfiltered = ctx.create_image_view(ImageViewInfo::new(
                    input.unfiltered.clone(),
                ))?;
                self.unfiltered[fid as usize].get_or_insert(unfiltered)
            }
        };

        if update_set {
            ctx.update_descriptor_sets(
                bump.alloc([WriteDescriptorSet {
                    set,
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::CombinedImageSampler(bump.alloc(
                        [
                            (
                                normal_depth.clone(),
                                Layout::ShaderReadOnlyOptimal,
                                self.sampler.clone(),
                            ),
                            (
                                unfiltered.clone(),
                                Layout::ShaderReadOnlyOptimal,
                                self.sampler.clone(),
                            ),
                        ],
                    )),
                }]),
                &[],
            );
        }

        let mut encoder = ctx.queue.create_encoder()?;

        let mut render_pass_encoder = encoder.with_render_pass(
            &self.render_pass,
            framebuffer,
            &[ClearValue::Color(0.3, 0.4, 0.5, 1.0)],
        );

        render_pass_encoder.bind_graphics_pipeline(&self.pipeline);
        render_pass_encoder.bind_graphics_descriptor_sets(
            &self.pipeline_layout,
            0,
            std::slice::from_ref(set),
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
        drop(render_pass_encoder);
        ctx.queue.submit(wait, encoder.finish(), signal, fence);

        Ok(Output {
            filtered: filtered.info().image.clone(),
        })
    }
}

use crate::{
    access::supported_access,
    convert::*,
    descriptor::DescriptorSizes,
    handle::*,
    physical::{EruptFeatures, EruptProperties},
    swapchain::EruptSwapchain,
    EruptGraphics,
};

use bumpalo::{collections::Vec as BVec, Bump};

use erupt::{
    extensions::{
        khr_ray_tracing::{self as vkrt, KhrRayTracingDeviceLoaderExt as _},
        khr_swapchain as vksw,
    },
    make_version,
    vk1_0::{self, Vk10DeviceLoaderExt as _},
    vk1_2::{self, Vk12DeviceLoaderExt as _},
    DeviceLoader,
};
use illume::{
    arith_eq, arith_le, host_memory_space_overlow, out_of_host_memory,
    AccelerationStructure, AccelerationStructureInfo,
    AccelerationStructureLevel, Buffer, BufferInfo, BufferUsage, ColorBlend,
    CopyDescriptorSet, CreateImageError, CreateRenderPassError,
    CreateShaderModuleError, DescriptorSet, DescriptorSetInfo,
    DescriptorSetLayout, DescriptorSetLayoutFlags, DescriptorSetLayoutInfo,
    Descriptors, DeviceAddress, DeviceTrait, Fence, FenceInfo, Framebuffer,
    FramebufferInfo, GraphicsPipeline, GraphicsPipelineInfo, Image, ImageInfo,
    ImageView, ImageViewInfo, ImageViewKind, InvalidShader, MemoryUsageFlags,
    OutOfMemory, PipelineLayout, PipelineLayoutInfo, RayTracingPipeline,
    RayTracingPipelineInfo, RayTracingShaderGroupInfo, RenderPass,
    RenderPassInfo, Sampler, SamplerInfo, Semaphore, SemaphoreInfo,
    ShaderBindingTable, ShaderBindingTableInfo, ShaderLanguage, ShaderModule,
    ShaderModuleInfo, State, StridedBufferRegion, Surface, SurfaceError,
    Swapchain, WriteDescriptorSet, SMALLVEC_SUBPASSES,
};

use parking_lot::Mutex;
use slab::Slab;
use smallvec::SmallVec;
use std::{
    convert::{TryFrom as _, TryInto as _},
    ffi::CString,
    fmt::{self, Debug},
    mem::MaybeUninit,
    ops::Range,
    sync::Arc,
};

pub(super) struct EruptDevice {
    pub(super) logical: DeviceLoader,
    pub(super) physical: vk1_0::PhysicalDevice,
    pub(super) graphics: Arc<EruptGraphics>,
    pub(super) properties: EruptProperties,
    pub(super) features: EruptFeatures,
    pub(super) allocator: tvma::Allocator,

    // Sparce arrays of resources created by this device.
    // They will be destroyed once device is dropped.
    // Produced handles won't be usable after device destruction
    // so dangling raw handles there will not be accessible anymore.
    // This approach seems better than keeping device alive until all resources
    // are dropped, because leaks are safe and single leaked buffer or image
    // will keep whole device and consequently instance alive.
    buffers: Mutex<Slab<vk1_0::Buffer>>,
    // buffer_views: Mutex<Slab<vk1_0::BufferView>>,
    descriptor_pools: Mutex<Slab<vk1_0::DescriptorPool>>,
    descriptor_sets: Mutex<Slab<vk1_0::DescriptorSet>>,
    descriptor_set_layouts: Mutex<Slab<vk1_0::DescriptorSetLayout>>,
    fences: Mutex<Slab<vk1_0::Fence>>,
    framebuffers: Mutex<Slab<vk1_0::Framebuffer>>,
    images: Mutex<Slab<vk1_0::Image>>,
    image_views: Mutex<Slab<vk1_0::ImageView>>,
    pipelines: Mutex<Slab<vk1_0::Pipeline>>,
    pipeline_layouts: Mutex<Slab<vk1_0::PipelineLayout>>,
    render_passes: Mutex<Slab<vk1_0::RenderPass>>,
    semaphores: Mutex<Slab<vk1_0::Semaphore>>,
    shaders: Mutex<Slab<vk1_0::ShaderModule>>,
    acceleration_strucutres: Mutex<Slab<vkrt::AccelerationStructureKHR>>,
    samplers: Mutex<Slab<vk1_0::Sampler>>,
    pub(super) swapchains: Mutex<Slab<vksw::SwapchainKHR>>,
}

impl EruptDevice {
    pub(super) unsafe fn new(
        logical: DeviceLoader,
        physical: vk1_0::PhysicalDevice,
        graphics: Arc<EruptGraphics>,
        properties: EruptProperties,
        features: EruptFeatures,
    ) -> Self {
        EruptDevice {
            allocator: tvma::Allocator::new(
                tvma::Config {
                    dedicated_treshold_low: 4 * 1024 * 1024,
                    dedicated_treshold_high: 32 * 1024 * 1024,
                    line_size: 32 * 1024 * 1024,
                    min_chunk_block: 256,
                },
                &properties.memory,
            ),
            logical,
            physical,
            graphics,
            properties,
            features,

            // Numbers here are hints so no strong reasoning is required.
            buffers: Mutex::new(Slab::with_capacity(4096)),
            // buffer_views: Mutex::new(Slab::with_capacity(4096)),
            descriptor_pools: Mutex::new(Slab::with_capacity(64)),
            descriptor_sets: Mutex::new(Slab::with_capacity(1024)),
            descriptor_set_layouts: Mutex::new(Slab::with_capacity(64)),
            fences: Mutex::new(Slab::with_capacity(128)),
            framebuffers: Mutex::new(Slab::with_capacity(128)),
            images: Mutex::new(Slab::with_capacity(4096)),
            image_views: Mutex::new(Slab::with_capacity(4096)),
            pipelines: Mutex::new(Slab::with_capacity(128)),
            pipeline_layouts: Mutex::new(Slab::with_capacity(64)),
            render_passes: Mutex::new(Slab::with_capacity(32)),
            semaphores: Mutex::new(Slab::with_capacity(128)),
            shaders: Mutex::new(Slab::with_capacity(512)),
            swapchains: Mutex::new(Slab::with_capacity(32)),
            acceleration_strucutres: Mutex::new(Slab::with_capacity(1024)),
            samplers: Mutex::new(Slab::with_capacity(128)),
        }
    }

    pub(super) fn create_semaphore_raw(
        &self,
    ) -> Result<(vk1_0::Semaphore, usize), vk1_0::Result> {
        let semaphore = unsafe {
            self.logical.create_semaphore(
                &vk1_0::SemaphoreCreateInfo::default().builder(),
                None,
                None,
            )
        }
        .result()?;

        let index = self.semaphores.lock().insert(semaphore);

        Ok((semaphore, index))
    }
}

impl Debug for EruptDevice {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("EruptDevice")
                .field("logical", &self.logical.handle)
                .field("physical", &self.physical)
                .field("instance", &self.graphics)
                .finish()
        } else {
            Debug::fmt(&self.logical.handle, fmt)
        }
    }
}

impl DeviceTrait for EruptDevice {
    fn create_buffer(
        self: Arc<Self>,
        info: BufferInfo,
    ) -> Result<Buffer, OutOfMemory> {
        if info.usage.contains(BufferUsage::SHADER_DEVICE_ADDRESS) {
            assert_ne!(self.features.v12.buffer_device_address, 0);
        }

        let handle = unsafe {
            self.logical.create_buffer(
                &vk1_0::BufferCreateInfo::default()
                    .builder()
                    .size(info.size)
                    .usage(info.usage.to_erupt())
                    .sharing_mode(vk1_0::SharingMode::EXCLUSIVE),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let reqs = unsafe {
            self.logical.get_buffer_memory_requirements(handle, None)
        };

        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.allocator.alloc(
                &self.logical,
                reqs.size,
                (reqs.alignment - 1) | info.align,
                reqs.memory_type_bits,
                memory_usage_to_tvma(info.memory),
                tvma::Dedicated::Indifferent,
            )
        }
        .map_err(|_| {
            unsafe { self.logical.destroy_buffer(handle, None) }

            OutOfMemory
        })?;

        let result = unsafe {
            self.logical.bind_buffer_memory(
                handle,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        if let Err(err) = result {
            unsafe {
                self.logical.destroy_buffer(handle, None);

                self.allocator.dealloc(&self.logical, block);
            }

            return Err(oom_error_from_erupt(err));
        }

        let address = if info.usage.contains(BufferUsage::SHADER_DEVICE_ADDRESS)
        {
            Some(Option::unwrap(from_erupt(unsafe {
                self.logical.get_buffer_device_address(
                    &vk1_2::BufferDeviceAddressInfo::default()
                        .builder()
                        .buffer(handle),
                )
            })))
        } else {
            None
        };

        let buffer_index = self.buffers.lock().insert(handle);

        Ok(Buffer::make(
            EruptBuffer {
                handle,
                address,
                owner: Arc::downgrade(&self),
                block,
                index: buffer_index,
            },
            info,
        ))
    }

    fn create_buffer_static(
        self: Arc<Self>,
        info: BufferInfo,
        data: &[u8],
    ) -> Result<Buffer, OutOfMemory> {
        debug_assert!(arith_eq(info.size, data.len()));
        assert!(info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD
        ));

        let buffer = self.clone().create_buffer(info)?;

        let erupt_buffer = buffer.erupt_ref(&*self);

        unsafe {
            match erupt_buffer.block.map(&self.logical, 0, data.len()) {
                Ok(ptr) => {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        ptr.as_ptr(),
                        data.len(),
                    );

                    erupt_buffer.block.unmap(&self.logical);

                    Ok(buffer)
                }
                Err(tvma::MappingError::OutOfMemory { .. }) => Err(OutOfMemory),
                Err(tvma::MappingError::NonHostVisible)
                | Err(tvma::MappingError::OutOfBounds) => unreachable!(),
            }
        }
    }

    fn create_fence(self: Arc<Self>) -> Result<Fence, OutOfMemory> {
        let fence = unsafe {
            self.logical.create_fence(
                &vk1_0::FenceCreateInfo::default().builder(),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.fences.lock().insert(fence);

        Ok(Fence::make(
            EruptFence {
                handle: fence,
                owner: Arc::downgrade(&self),
                index,
            },
            FenceInfo,
        ))
    }

    fn create_framebuffer(
        self: Arc<Self>,
        info: FramebufferInfo,
    ) -> Result<Framebuffer, OutOfMemory> {
        assert!(
            info.views.iter().all(|view| view.is_owner(&*self)),
            "Wrong owner"
        );

        assert!(
            info.views
                .iter()
                .all(|view| view.info().view_kind == ImageViewKind::D2),
            "Image views for Framebuffer must all has `view_kind == ImageViewKind::D2`",
        );

        assert!(
            info.views
                .iter()
                .all(|view| { view.info().image.info().extent.into_2d() >= info.extent }),
            "Image views for Framebuffer must be at least as large as framebuffer extent",
        );

        let attachments = info
            .views
            .iter()
            .map(|view| view.erupt_ref(&*self).handle)
            .collect::<SmallVec<[_; 16]>>();

        let framebuffer = unsafe {
            self.logical.create_framebuffer(
                &vk1_0::FramebufferCreateInfo::default()
                    .builder()
                    .render_pass(info.render_pass.erupt_ref(&*self).handle)
                    .attachments(&attachments)
                    .width(info.extent.width)
                    .height(info.extent.height)
                    .layers(1),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.framebuffers.lock().insert(framebuffer);

        Ok(Framebuffer::make(
            EruptFramebuffer {
                handle: framebuffer,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_graphics_pipeline(
        self: Arc<Self>,
        info: GraphicsPipelineInfo,
    ) -> Result<GraphicsPipeline, OutOfMemory> {
        let bump = Bump::new();
        let vertex_shader_entry: CString;
        let fragment_shader_entry: CString;
        let mut shader_stages = BVec::with_capacity_in(2, &bump);
        let mut dynamic_states = BVec::with_capacity_in(7, &bump);

        let vertex_binding_descriptions = info
            .vertex_bindings
            .iter()
            .enumerate()
            .map(|(i, vb)| {
                vk1_0::VertexInputBindingDescription::default()
                    .builder()
                    .binding(i.try_into().unwrap())
                    .stride(vb.stride)
                    .input_rate(vb.rate.to_erupt())
            })
            .collect::<SmallVec<[_; 16]>>();

        let vertex_attribute_descriptions = info
            .vertex_attributes
            .iter()
            .map(|attr| {
                vk1_0::VertexInputAttributeDescription::default()
                    .builder()
                    .location(attr.location)
                    .binding(attr.binding)
                    .offset(attr.offset)
                    .format(attr.format.to_erupt())
            })
            .collect::<SmallVec<[_; 16]>>();

        let vertex_input_state =
            vk1_0::PipelineVertexInputStateCreateInfo::default()
                .builder()
                .vertex_binding_descriptions(&vertex_binding_descriptions)
                .vertex_attribute_descriptions(&vertex_attribute_descriptions);

        vertex_shader_entry = entry_name_to_cstr(info.vertex_shader.entry());

        shader_stages.push(
            vk1_0::PipelineShaderStageCreateInfo::default()
                .builder()
                .stage(vk1_0::ShaderStageFlagBits::VERTEX)
                .module(info.vertex_shader.module().erupt_ref(&*self).handle)
                .name(&*vertex_shader_entry),
        );

        let input_assembly_state =
            vk1_0::PipelineInputAssemblyStateCreateInfo::default()
                .builder()
                .topology(info.primitive_topology.to_erupt())
                .primitive_restart_enable(info.primitive_restart_enable);

        let rasterization_state;

        let viewport;

        let scissor;

        let mut viewport_state = None;

        let mut multisample_state = None;

        let mut depth_stencil_state = None;

        let mut color_blend_state = None;

        let with_rasterizer = if let Some(rasterizer) = &info.rasterizer {
            let mut builder =
                vk1_0::PipelineViewportStateCreateInfo::default().builder();

            match &rasterizer.viewport {
                State::Static { value } => {
                    viewport = value.to_erupt().builder();

                    builder =
                        builder.viewports(std::slice::from_ref(&viewport));
                }
                State::Dynamic => {
                    dynamic_states.push(vk1_0::DynamicState::VIEWPORT)
                }
            }

            match &rasterizer.scissor {
                State::Static { value } => {
                    scissor = value.to_erupt().builder();

                    builder = builder.scissors(std::slice::from_ref(&scissor));
                }
                State::Dynamic => {
                    dynamic_states.push(vk1_0::DynamicState::SCISSOR)
                }
            }

            viewport_state = Some(builder);

            rasterization_state =
                vk1_0::PipelineRasterizationStateCreateInfo::default()
                    .builder()
                    .rasterizer_discard_enable(false)
                    .depth_clamp_enable(rasterizer.depth_clamp)
                    .polygon_mode(rasterizer.polygon_mode.to_erupt())
                    .cull_mode(rasterizer.culling.to_erupt())
                    .front_face(rasterizer.front_face.to_erupt())
                    .line_width(1.0);

            multisample_state = Some(
                vk1_0::PipelineMultisampleStateCreateInfo::default()
                    .builder()
                    .rasterization_samples(vk1_0::SampleCountFlagBits::_1),
            );

            let mut builder =
                vk1_0::PipelineDepthStencilStateCreateInfo::default().builder();

            if let Some(depth_test) = rasterizer.depth_test {
                builder = builder
                    .depth_test_enable(true)
                    .depth_write_enable(depth_test.write)
                    .depth_compare_op(depth_test.compare.to_erupt())
            };

            if let Some(depth_bounds) = rasterizer.depth_bounds {
                builder = builder.depth_bounds_test_enable(true);

                match depth_bounds {
                    State::Static { value } => {
                        builder = builder
                            .min_depth_bounds(value.offset.into())
                            .max_depth_bounds(
                                value.offset.into_inner()
                                    + value.size.into_inner(),
                            )
                    }
                    State::Dynamic => {
                        dynamic_states.push(vk1_0::DynamicState::DEPTH_BOUNDS)
                    }
                }
            }

            if let Some(stencil_tests) = rasterizer.stencil_tests {
                builder = builder
                    .stencil_test_enable(true)
                    .front({
                        let mut builder = vk1_0::StencilOpState::default()
                            .builder()
                            .fail_op(stencil_tests.front.fail.to_erupt())
                            .pass_op(stencil_tests.front.pass.to_erupt())
                            .depth_fail_op(
                                stencil_tests.front.depth_fail.to_erupt(),
                            )
                            .compare_op(stencil_tests.front.compare.to_erupt());

                        match stencil_tests.front.compare_mask {
                            State::Static { value } => {
                                builder = builder.compare_mask(value)
                            }
                            State::Dynamic => dynamic_states.push(
                                vk1_0::DynamicState::STENCIL_COMPARE_MASK,
                            ),
                        }

                        match stencil_tests.front.write_mask {
                            State::Static { value } => {
                                builder = builder.write_mask(value)
                            }
                            State::Dynamic => dynamic_states
                                .push(vk1_0::DynamicState::STENCIL_WRITE_MASK),
                        }

                        match stencil_tests.front.reference {
                            State::Static { value } => {
                                builder = builder.reference(value)
                            }
                            State::Dynamic => dynamic_states
                                .push(vk1_0::DynamicState::STENCIL_REFERENCE),
                        }

                        *builder
                    })
                    .back({
                        let mut builder = vk1_0::StencilOpState::default()
                            .builder()
                            .fail_op(stencil_tests.back.fail.to_erupt())
                            .pass_op(stencil_tests.back.pass.to_erupt())
                            .depth_fail_op(
                                stencil_tests.back.depth_fail.to_erupt(),
                            )
                            .compare_op(stencil_tests.back.compare.to_erupt());

                        match stencil_tests.back.compare_mask {
                            State::Static { value } => {
                                builder = builder.compare_mask(value)
                            }
                            State::Dynamic => dynamic_states.push(
                                vk1_0::DynamicState::STENCIL_COMPARE_MASK,
                            ),
                        }

                        match stencil_tests.back.write_mask {
                            State::Static { value } => {
                                builder = builder.write_mask(value)
                            }
                            State::Dynamic => dynamic_states
                                .push(vk1_0::DynamicState::STENCIL_WRITE_MASK),
                        }

                        match stencil_tests.back.reference {
                            State::Static { value } => {
                                builder = builder.reference(value)
                            }
                            State::Dynamic => dynamic_states
                                .push(vk1_0::DynamicState::STENCIL_REFERENCE),
                        }

                        *builder
                    });
            }

            depth_stencil_state = Some(builder);

            if let Some(shader) = &rasterizer.fragment_shader {
                fragment_shader_entry = entry_name_to_cstr(shader.entry());
                shader_stages.push(
                    vk1_0::PipelineShaderStageCreateInfo::default()
                        .builder()
                        .stage(vk1_0::ShaderStageFlagBits::FRAGMENT)
                        .module(shader.module().erupt_ref(&*self).handle)
                        .name(&*fragment_shader_entry),
                );
            }

            let mut builder =
                vk1_0::PipelineColorBlendStateCreateInfo::default().builder();

            builder = match rasterizer.color_blend {
                ColorBlend::Logic { op } => {
                    builder.logic_op_enable(true).logic_op(op.to_erupt())
                }
                ColorBlend::Blending {
                    blending,
                    write_mask,
                    constants,
                } => {
                    builder = builder.logic_op_enable(false).attachments({
                        bump.alloc_slice_fill_iter(
                            (0..info.render_pass.info().attachments.len()).map(|_| {
                                if let Some(blending) = blending {
                                    vk1_0::PipelineColorBlendAttachmentState::default()
                                        .builder()
                                        .blend_enable(true)
                                        .src_color_blend_factor(
                                            blending.color_src_factor.to_erupt(),
                                        )
                                        .dst_color_blend_factor(
                                            blending.color_dst_factor.to_erupt(),
                                        )
                                        .color_blend_op(blending.color_op.to_erupt())
                                        .src_alpha_blend_factor(
                                            blending.alpha_src_factor.to_erupt(),
                                        )
                                        .dst_alpha_blend_factor(
                                            blending.alpha_dst_factor.to_erupt(),
                                        )
                                        .alpha_blend_op(blending.alpha_op.to_erupt())
                                } else {
                                    vk1_0::PipelineColorBlendAttachmentState::default()
                                        .builder()
                                        .blend_enable(false)
                                }
                                .color_write_mask(write_mask.to_erupt())
                            }),
                        )
                    });

                    match constants {
                        State::Static {
                            value: [x, y, z, w],
                        } => {
                            builder = builder.blend_constants([
                                x.into(),
                                y.into(),
                                z.into(),
                                w.into(),
                            ])
                        }
                        State::Dynamic => dynamic_states
                            .push(vk1_0::DynamicState::BLEND_CONSTANTS),
                    }

                    builder
                }

                ColorBlend::IndependentBlending { .. } => {
                    panic!("Unsupported yet")
                }
            };

            color_blend_state = Some(builder);

            true
        } else {
            rasterization_state =
                vk1_0::PipelineRasterizationStateCreateInfo::default()
                    .builder()
                    .rasterizer_discard_enable(true);

            false
        };

        let mut builder = vk1_0::GraphicsPipelineCreateInfo::default()
            .builder()
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .rasterization_state(&rasterization_state)
            .stages(&shader_stages)
            .layout(info.layout.erupt_ref(&*self).handle)
            .render_pass(info.render_pass.erupt_ref(&*self).handle)
            .subpass(info.subpass);

        let pipeline_dynamic_state;

        if !dynamic_states.is_empty() {
            pipeline_dynamic_state =
                vk1_0::PipelineDynamicStateCreateInfo::default()
                    .builder()
                    .dynamic_states(&dynamic_states);

            builder = builder.dynamic_state(&pipeline_dynamic_state);
        }

        if with_rasterizer {
            builder = builder
                .viewport_state(viewport_state.as_ref().unwrap())
                .multisample_state(multisample_state.as_ref().unwrap())
                .color_blend_state(color_blend_state.as_ref().unwrap())
                .depth_stencil_state(depth_stencil_state.as_ref().unwrap());
        }

        let pipelines = unsafe {
            self.logical.create_graphics_pipelines(
                vk1_0::PipelineCache::null(),
                &[builder],
                None,
            )
        }
        .result()
        .map_err(|err| oom_error_from_erupt(err))?;

        debug_assert_eq!(pipelines.len(), 1);

        let pipeline = pipelines[0];

        let index = self.pipelines.lock().insert(pipeline);

        drop(shader_stages);

        Ok(GraphicsPipeline::make(
            EruptGraphicsPipeline {
                handle: pipeline,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_image(
        self: Arc<Self>,
        info: ImageInfo,
    ) -> Result<Image, CreateImageError> {
        let image = unsafe {
            self.logical.create_image(
                &vk1_0::ImageCreateInfo::default()
                    .builder()
                    .image_type(info.extent.to_erupt())
                    .format(info.format.to_erupt())
                    .extent(info.extent.into_3d().to_erupt())
                    .mip_levels(info.levels)
                    .array_layers(info.layers)
                    .samples(info.samples.to_erupt())
                    .tiling(vk1_0::ImageTiling::OPTIMAL)
                    .usage(info.usage.to_erupt())
                    .sharing_mode(vk1_0::SharingMode::EXCLUSIVE)
                    .initial_layout(vk1_0::ImageLayout::UNDEFINED),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let reqs =
            unsafe { self.logical.get_image_memory_requirements(image, None) };

        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.allocator
                .alloc(
                    &self.logical,
                    reqs.size,
                    reqs.alignment - 1,
                    reqs.memory_type_bits,
                    memory_usage_to_tvma(info.memory),
                    tvma::Dedicated::Indifferent,
                )
                .map_err(|_| {
                    self.logical.destroy_image(image, None);

                    OutOfMemory
                })
        }?;

        let result = unsafe {
            self.logical.bind_image_memory(
                image,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        match result {
            Ok(()) => {
                let index = self.images.lock().insert(image);

                Ok(Image::make(
                    EruptImage {
                        handle: image,
                        owner: Arc::downgrade(&self),
                        block: Some(block),
                        index,
                    },
                    info,
                ))
            }
            Err(err) => {
                unsafe {
                    self.logical.destroy_image(image, None);
                    self.allocator.dealloc(&self.logical, block);
                }

                Err(oom_error_from_erupt(err).into())
            }
        }
    }

    fn create_image_static(
        self: Arc<Self>,
        info: ImageInfo,
        data: &[u8],
    ) -> Result<Image, CreateImageError> {
        assert!(info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD
        ));

        let image = unsafe {
            self.logical.create_image(
                &vk1_0::ImageCreateInfo::default()
                    .builder()
                    .image_type(info.extent.to_erupt())
                    .format(info.format.to_erupt())
                    .extent(info.extent.into_3d().to_erupt())
                    .mip_levels(info.levels)
                    .array_layers(info.layers)
                    .samples(info.samples.to_erupt())
                    .tiling(vk1_0::ImageTiling::LINEAR)
                    .usage(info.usage.to_erupt())
                    .sharing_mode(vk1_0::SharingMode::EXCLUSIVE)
                    .initial_layout(vk1_0::ImageLayout::UNDEFINED),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let reqs =
            unsafe { self.logical.get_image_memory_requirements(image, None) };

        debug_assert!(arith_eq(reqs.size, data.len()));
        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.allocator
                .alloc(
                    &self.logical,
                    reqs.size,
                    reqs.alignment - 1,
                    reqs.memory_type_bits,
                    memory_usage_to_tvma(info.memory),
                    tvma::Dedicated::Indifferent,
                )
                .map_err(|_| {
                    self.logical.destroy_image(image, None);

                    OutOfMemory
                })
        }?;

        let result = unsafe {
            self.logical.bind_image_memory(
                image,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        if let Err(err) = result {
            unsafe {
                self.logical.destroy_image(image, None);
                self.allocator.dealloc(&self.logical, block);
            }
            return Err(oom_error_from_erupt(err).into());
        }

        unsafe {
            match block.map(&self.logical, 0, data.len()) {
                Ok(ptr) => {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        ptr.as_ptr(),
                        data.len(),
                    );

                    block.unmap(&self.logical);
                }
                Err(tvma::MappingError::OutOfMemory { .. }) => {
                    return Err(OutOfMemory.into());
                }
                Err(tvma::MappingError::NonHostVisible)
                | Err(tvma::MappingError::OutOfBounds) => unreachable!(),
            }
        }

        let index = self.images.lock().insert(image);

        Ok(Image::make(
            EruptImage {
                handle: image,
                owner: Arc::downgrade(&self),
                block: Some(block),
                index,
            },
            info,
        ))
    }

    fn create_image_view(
        self: Arc<Self>,
        info: ImageViewInfo,
    ) -> Result<ImageView, OutOfMemory> {
        let image = info.image.erupt_ref(&*self);

        let view = unsafe {
            self.logical.create_image_view(
                &vk1_0::ImageViewCreateInfo::default()
                    .builder()
                    .image(image.handle)
                    .format(info.image.info().format.to_erupt())
                    .view_type(info.view_kind.to_erupt())
                    .subresource_range(
                        vk1_0::ImageSubresourceRange::default()
                            .builder()
                            .aspect_mask(info.subresource.aspect.to_erupt())
                            .base_mip_level(info.subresource.first_level)
                            .level_count(info.subresource.level_count)
                            .base_array_layer(info.subresource.first_layer)
                            .layer_count(info.subresource.layer_count)
                            .discard(),
                    ),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.image_views.lock().insert(view);

        Ok(ImageView::make(
            EruptImageView {
                handle: view,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_pipeline_layout(
        self: Arc<Self>,
        info: PipelineLayoutInfo,
    ) -> Result<PipelineLayout, OutOfMemory> {
        let pipeline_layout = unsafe {
            self.logical.create_pipeline_layout(
                &vk1_0::PipelineLayoutCreateInfo::default()
                    .builder()
                    .set_layouts(
                        &info
                            .sets
                            .iter()
                            .map(|set| set.erupt_ref(&*self).handle)
                            .collect::<SmallVec<[_; 16]>>(),
                    ),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.pipeline_layouts.lock().insert(pipeline_layout);

        Ok(PipelineLayout::make(
            EruptPipelineLayout {
                handle: pipeline_layout,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_render_pass(
        self: Arc<Self>,
        info: RenderPassInfo,
    ) -> Result<RenderPass, CreateRenderPassError> {
        let mut subpass_attachments = Vec::new();

        let subpasses =
            info.subpasses
                .iter()
                .enumerate()
                .map(|(si, s)| -> Result<_, CreateRenderPassError> {
                    let color_offset = subpass_attachments.len();
                    subpass_attachments.extend(
                        s.colors
                            .iter()
                            .enumerate()
                            .map(|(ci, &c)| -> Result<_, CreateRenderPassError> {
                                Ok(vk1_0::AttachmentReference::default().builder()
                                .attachment(if c < info.attachments.len() {
                                    Some(c)
                                } else {
                                    None
                                }
                                .and_then(|c| c.try_into().ok())
                                .ok_or_else(|| {
                                    CreateRenderPassError::ColorAttachmentReferenceOutOfBound {
                                        subpass: si,
                                        index: ci,
                                        attachment: c,
                                    }
                                })?)
                                .layout(vk1_0::ImageLayout::GENERAL)
                            )
                            })
                            .collect::<Result<SmallVec<[_; 16]>, _>>()?,
                    );

                    let depth_offset = subpass_attachments.len();
                    if let Some(d) = s.depth {
                        subpass_attachments.push(
                            vk1_0::AttachmentReference::default()
                                .builder()
                                .attachment(
                                    if d < info.attachments.len() {
                                        Some(d)
                                    } else {
                                        None
                                    }
                                    .and_then(|d| d.try_into().ok())
                                    .ok_or_else(|| {
                                        CreateRenderPassError::DepthAttachmentReferenceOutOfBound {
                                            subpass: si,
                                            attachment: d,
                                        }
                                    })?,
                                )
                                .layout(vk1_0::ImageLayout::GENERAL),
                        );
                    }
                    Ok((color_offset, depth_offset))
                })
                .collect::<Result<SmallVec<[_; 16]>, _>>()?;

        let subpasses = info
            .subpasses
            .iter()
            .zip(subpasses)
            .map(|(s, (color_offset, depth_offset))| {
                let builder = vk1_0::SubpassDescription::default()
                    .builder()
                    .color_attachments(
                        &subpass_attachments[color_offset..depth_offset],
                    );

                if s.depth.is_some() {
                    builder.depth_stencil_attachment(
                        &subpass_attachments[depth_offset],
                    )
                } else {
                    builder
                }
            })
            .collect::<SmallVec<[_; SMALLVEC_SUBPASSES]>>();

        let attachments = info
            .attachments
            .iter()
            .map(|a| {
                vk1_0::AttachmentDescription::default()
                    .builder()
                    .format(a.format.to_erupt())
                    .load_op(a.load_op.to_erupt())
                    .store_op(a.store_op.to_erupt())
                    .initial_layout(a.initial_layout.to_erupt())
                    .final_layout(a.final_layout.to_erupt())
                    .samples(vk1_0::SampleCountFlagBits::_1)
            })
            .collect::<SmallVec<[_; 16]>>();

        let dependencies = info
            .dependencies
            .iter()
            .map(|d| {
                vk1_0::SubpassDependency::default()
                    .builder()
                    .src_subpass(
                        d.src
                            .map(|s| {
                                s.try_into()
                                    .expect("Subpass index out of bound")
                            })
                            .unwrap_or(vk1_0::SUBPASS_EXTERNAL),
                    )
                    .dst_subpass(
                        d.dst
                            .map(|s| {
                                s.try_into()
                                    .expect("Subpass index out of bound")
                            })
                            .unwrap_or(vk1_0::SUBPASS_EXTERNAL),
                    )
                    .src_stage_mask(d.src_stages.to_erupt())
                    .dst_stage_mask(d.dst_stages.to_erupt())
                    .src_access_mask(supported_access(d.src_stages.to_erupt()))
                    .dst_access_mask(supported_access(d.dst_stages.to_erupt()))
            })
            .collect::<SmallVec<[_; 16]>>();

        let render_passs_create_info = vk1_0::RenderPassCreateInfo::default()
            .builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        let render_pass = unsafe {
            self.logical.create_render_pass(
                &render_passs_create_info,
                None,
                None,
            )
        }
        .result()
        .map_err(create_render_pass_error_from_erupt)?;

        let index = self.render_passes.lock().insert(render_pass);

        Ok(RenderPass::make(
            EruptRenderPass {
                handle: render_pass,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_shader_module(
        self: Arc<Self>,
        info: ShaderModuleInfo,
    ) -> Result<ShaderModule, CreateShaderModuleError> {
        #[cfg(feature = "shader-compiler")]
        let code: Box<[u8]>;

        let code = &match info.language {
            ShaderLanguage::SPIRV => &info.code,

            #[cfg(feature = "shader-compiler")]
            _ => {
                code = shader_compiler::compile_shader(
                    &info.code,
                    "main",
                    info.language,
                )
                .map_err(|err| {
                    CreateShaderModuleError::Other {
                        source: Box::new(err),
                    }
                })?;

                &code
            }

            #[cfg(not(feature = "shader-compiler"))]
            _ => {
                return Err(
                    CreateShaderModuleError::UnsupportedShaderLanguage {
                        language: info.language,
                    },
                )
            }
        };

        if code.len() == 0 {
            return Err(CreateShaderModuleError::InvalidShader {
                source: InvalidShader::EmptySource,
            });
        }

        if code.len() & 3 > 0 {
            return Err(CreateShaderModuleError::InvalidShader {
                source: InvalidShader::SizeIsNotMultipleOfFour,
            });
        }

        let magic: u32 = unsafe {
            // The size is at least 4 bytes.
            std::ptr::read_unaligned(code.as_ptr() as *const u32)
        };

        if magic != 0x07230203 {
            return Err(CreateShaderModuleError::InvalidShader {
                source: InvalidShader::WrongMagic { found: magic },
            });
        }

        let mut aligned_code;

        let is_aligned = code.as_ptr() as usize & 3 == 0;

        let code_slice = if !is_aligned {
            // Copy spirv code into aligned array.
            unsafe {
                aligned_code = Vec::<u32>::with_capacity(code.len() / 4);

                // Copying array of `u8` into 4 times smaller array of `u32`.
                // They cannot overlap.
                std::ptr::copy_nonoverlapping(
                    code.as_ptr(),
                    aligned_code.as_mut_ptr() as *mut u8,
                    code.len(),
                );

                // Those values are initialized by copy operation above.
                aligned_code.set_len(code.len() / 4);
            }

            &aligned_code[..]
        } else {
            unsafe {
                // As `[u8; 4]` must be compatible with `u32`
                // `[u8; N]` must be compatible with `[u32; N / 4]
                // Resulting lifetime is bound to the function while
                // source lifetime is not less than the function.
                std::slice::from_raw_parts(
                    code.as_ptr() as *const u32,
                    code.len() / 4,
                )
            }
        };

        let module = unsafe {
            // FIXME: It is still required to validate SPIR-V.
            // Othewise adheres to valid usage described in spec.
            self.logical.create_shader_module(
                &vk1_0::ShaderModuleCreateInfo::default()
                    .builder()
                    .code(code_slice),
                None,
                None,
            )
        }
        .result()
        .map_err(|err| CreateShaderModuleError::OutOfMemoryError {
            source: oom_error_from_erupt(err),
        })?;

        let index = self.shaders.lock().insert(module);

        Ok(ShaderModule::make(
            EruptShaderModule {
                handle: module,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn create_semaphore(self: Arc<Self>) -> Result<Semaphore, OutOfMemory> {
        let (handle, index) =
            self.create_semaphore_raw().map_err(oom_error_from_erupt)?;

        Ok(Semaphore::make(
            EruptSemaphore {
                handle,
                owner: Arc::downgrade(&self),
                index,
            },
            SemaphoreInfo,
        ))
    }

    fn create_swapchain(
        self: Arc<Self>,
        surface: &mut Surface,
    ) -> Result<Swapchain, SurfaceError> {
        Ok(Swapchain::new(Box::new(EruptSwapchain::new(
            surface, &self,
        )?)))
    }

    fn reset_fences(&self, fences: &[&Fence]) {
        let fences = fences
            .iter()
            .map(|fence| fence.erupt_ref(self).handle)
            .collect::<SmallVec<[_; 16]>>();

        unsafe { self.logical.reset_fences(&fences) }
            .expect("TODO: Handle device lost")
    }

    fn is_fence_signalled(&self, fence: &Fence) -> bool {
        let fence = fence.erupt_ref(self).handle;

        match unsafe { self.logical.get_fence_status(fence) }.raw {
            vk1_0::Result::SUCCESS => true,
            vk1_0::Result::NOT_READY => true,
            vk1_0::Result::ERROR_DEVICE_LOST => panic!("Device lost"),
            err => panic!("Unexpected error: {}", err),
        }
    }

    fn wait_fences(&self, fences: &[&Fence], all: bool) {
        let fences = fences
            .iter()
            .map(|fence| fence.erupt_ref(self).handle)
            .collect::<SmallVec<[_; 16]>>();

        unsafe { self.logical.wait_for_fences(&fences, all, !0) }
            .expect("TODO: Handle device lost")
    }

    fn wait_idle(&self) {
        unsafe {
            self.logical
                .device_wait_idle()
                .expect("TODO: Handle device lost")
        }
    }

    fn create_acceleration_structure(
        self: Arc<Self>,
        info: AccelerationStructureInfo,
    ) -> Result<AccelerationStructure, OutOfMemory> {
        assert!(
            self.logical.khr_ray_tracing.is_some(),
            "RayTracing feature is not enabled"
        );

        // assert_ne!(info.geometries.len(), 0);

        assert!(
            arith_le(
                info.geometries.len(),
                self.properties.rt.max_geometry_count
            ),
            "Too many gemetries: {}. Limit: {}",
            info.geometries.len(),
            self.properties.rt.max_geometry_count
        );

        let level = info.level;

        let geometries: SmallVec<[_; 16]> = info
            .geometries
            .iter()
            .copied()
            .inspect(|geometry| match level {
                AccelerationStructureLevel::Bottom => {
                    assert!(!geometry.is_instances());
                }
                AccelerationStructureLevel::Top => {
                    assert!(geometry.is_instances());
                }
            })
            .map(|geomery| geomery.to_erupt().builder())
            .collect();

        let handle = unsafe {
            self.logical.create_acceleration_structure_khr(
                &vkrt::AccelerationStructureCreateInfoKHR::default()
                    .builder()
                    ._type(info.level.to_erupt())
                    .flags(info.flags.to_erupt())
                    .geometry_infos(&geometries),
                None,
                None,
            )
        }
        .result()
        .map_err(|result| match result {
            vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => out_of_host_memory(),
            vk1_0::Result::ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS_KHR => panic!(
                "INVALID_OPAQUE_CAPTURE_ADDRESS_KHR error was unexpected"
            ),
            _ => panic!("Unexpected result {}", result),
        })?;

        let reqs = unsafe {
            self.logical
                .get_acceleration_structure_memory_requirements_khr(
                    &vkrt::AccelerationStructureMemoryRequirementsInfoKHR::default()
                        .builder()
                        ._type(vkrt::AccelerationStructureMemoryRequirementsTypeKHR::OBJECT_KHR)
                        .build_type(vkrt::AccelerationStructureBuildTypeKHR(1)) // TODO: Use assocated constant.
                        .acceleration_structure(handle),
                    None,
                )
        }
        .memory_requirements;

        tracing::debug!(
            "Acceleration structure memory requirements {:#?}",
            reqs
        );

        let block = unsafe {
            self.allocator.alloc(
                &self.logical,
                reqs.size,
                reqs.alignment - 1,
                reqs.memory_type_bits,
                tvma::UsageFlags::empty(),
                tvma::Dedicated::Indifferent,
            )
        }
        .map_err(|err| {
            unsafe {
                self.logical
                    .destroy_acceleration_structure_khr(handle, None);
            }
            tracing::error!("{}", err);
            OutOfMemory
        })?;

        let result = unsafe {
            self.logical.bind_acceleration_structure_memory_khr(&[
                vkrt::BindAccelerationStructureMemoryInfoKHR::default()
                    .builder()
                    .acceleration_structure(handle)
                    .memory(block.memory())
                    .memory_offset(block.offset()),
            ])
        }
        .result();

        match result {
            Ok(()) => {
                let index = self.acceleration_strucutres.lock().insert(handle);

                let address = Option::unwrap(from_erupt(unsafe {
                    self.logical.get_acceleration_structure_device_address_khr(
                        &vkrt::AccelerationStructureDeviceAddressInfoKHR::default()
                            .builder()
                            .acceleration_structure(handle),
                    )
                }));

                Ok(AccelerationStructure::make(
                    EruptAccelerationStructure {
                        handle,
                        address,
                        block,
                        owner: Arc::downgrade(&self),
                        index,
                    },
                    info,
                ))
            }
            Err(err) => {
                unsafe {
                    self.logical
                        .destroy_acceleration_structure_khr(handle, None);
                    self.allocator.dealloc(&self.logical, block);
                }

                Err(oom_error_from_erupt(err).into())
            }
        }
    }

    fn get_buffer_device_address(
        &self,
        buffer: &Buffer,
    ) -> Option<DeviceAddress> {
        if buffer
            .info()
            .usage
            .contains(BufferUsage::SHADER_DEVICE_ADDRESS)
        {
            assert_ne!(self.features.v12.buffer_device_address, 0);

            Some(buffer.erupt_ref(self).address.expect(
                "Device address for buffer must be set when `BufferUsage::SHADER_DEVICE_ADDRESS` is specified",
            ))
        } else {
            None
        }
    }

    fn get_acceleration_structure_device_address(
        &self,
        acceleration_structure: &AccelerationStructure,
    ) -> DeviceAddress {
        acceleration_structure.erupt_ref(self).address
    }

    fn allocate_acceleration_structure_build_scratch(
        self: Arc<Self>,
        acceleration_structure: &AccelerationStructure,
        update: bool,
    ) -> Result<Buffer, OutOfMemory> {
        assert!(
            self.logical.khr_ray_tracing.is_some(),
            "RayTracing feature is not enabled"
        );

        // Collect memory requirements.
        let size = unsafe {
            self.logical
                .get_acceleration_structure_memory_requirements_khr(
                    &vkrt::AccelerationStructureMemoryRequirementsInfoKHR::default()
                        .builder()
                        ._type(if update {
                            vkrt::AccelerationStructureMemoryRequirementsTypeKHR::UPDATE_SCRATCH_KHR
                        } else {
                            vkrt::AccelerationStructureMemoryRequirementsTypeKHR::BUILD_SCRATCH_KHR
                        })
                        .build_type(vkrt::AccelerationStructureBuildTypeKHR(1)) // TODO: Use assocated constant.
                        .acceleration_structure(acceleration_structure.erupt_ref(&*self).handle),
                    None,
                )
        }
        .memory_requirements
        .size;

        // Allocate memory.
        self.create_buffer(BufferInfo {
            align: 0,
            size,
            usage: BufferUsage::RAY_TRACING
                | BufferUsage::SHADER_DEVICE_ADDRESS,
            memory: MemoryUsageFlags::empty(),
        })
    }

    fn create_ray_tracing_pipeline(
        self: Arc<Self>,
        info: RayTracingPipelineInfo,
    ) -> Result<RayTracingPipeline, OutOfMemory> {
        assert!(
            self.logical.khr_ray_tracing.is_some(),
            "RayTracing feature is not enabled"
        );

        let entries: Vec<_> = info
            .shaders
            .iter()
            .map(|shader| entry_name_to_cstr(shader.entry()))
            .collect();

        let mut entries = entries.iter();

        let stages: Vec<_> = info
            .shaders
            .iter()
            .map(|shader| {
                vk1_0::PipelineShaderStageCreateInfo::default()
                    .builder()
                    .stage(shader.stage().to_erupt())
                    .module(shader.module().erupt_ref(&*self).handle)
                    .name(entries.next().unwrap())
            })
            .collect();

        let groups: Vec<_> = info
            .groups
            .iter()
            .map(|group| {
                let builder = vkrt::RayTracingShaderGroupCreateInfoKHR::default().builder();
                match *group {
                    RayTracingShaderGroupInfo::Raygen { raygen } => builder
                        ._type(vkrt::RayTracingShaderGroupTypeKHR::GENERAL_KHR)
                        .general_shader(raygen)
                        .any_hit_shader(vkrt::SHADER_UNUSED_KHR)
                        .closest_hit_shader(vkrt::SHADER_UNUSED_KHR)
                        .intersection_shader(vkrt::SHADER_UNUSED_KHR),
                    RayTracingShaderGroupInfo::Miss { miss } => builder
                        ._type(vkrt::RayTracingShaderGroupTypeKHR::GENERAL_KHR)
                        .general_shader(miss)
                        .any_hit_shader(vkrt::SHADER_UNUSED_KHR)
                        .closest_hit_shader(vkrt::SHADER_UNUSED_KHR)
                        .intersection_shader(vkrt::SHADER_UNUSED_KHR),
                    RayTracingShaderGroupInfo::Triangles {
                        any_hit,
                        closest_hit,
                    } => builder
                        ._type(vkrt::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP_KHR)
                        .general_shader(vkrt::SHADER_UNUSED_KHR)
                        .any_hit_shader(any_hit.unwrap_or(vkrt::SHADER_UNUSED_KHR))
                        .closest_hit_shader(closest_hit.unwrap_or(vkrt::SHADER_UNUSED_KHR))
                        .intersection_shader(vkrt::SHADER_UNUSED_KHR),
                }
            })
            .collect();

        let handles = unsafe {
            self.logical.create_ray_tracing_pipelines_khr(
                vk1_0::PipelineCache::null(),
                &[vkrt::RayTracingPipelineCreateInfoKHR::default()
                    .builder()
                    .stages(&stages)
                    .groups(&groups)
                    .max_recursion_depth(info.max_recursion_depth)
                    .layout(info.layout.erupt_ref(&*self).handle)],
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        assert_eq!(handles.len(), 1);

        let handle = handles[0];

        let group_size = self.properties.rt.shader_group_handle_size;

        let group_size_usize =
            usize::try_from(group_size).map_err(|_| out_of_host_memory())?;

        let total_size_usize = group_size_usize
            .checked_mul(info.groups.len())
            .ok_or_else(host_memory_space_overlow)?;

        let group_count =
            u32::try_from(info.groups.len()).map_err(|_| OutOfMemory)?;

        let mut bytes = vec![0u8; total_size_usize];

        unsafe {
            self.logical.get_ray_tracing_shader_group_handles_khr(
                handle,
                0,
                group_count,
                bytes.len(),
                bytes.as_mut_ptr() as *mut _,
            )
        }
        .result()
        .map_err(|err| {
            unsafe { self.logical.destroy_pipeline(handle, None) }

            oom_error_from_erupt(err)
        })?;

        let index = self.pipelines.lock().insert(handle);

        Ok(RayTracingPipeline::make(
            EruptRayTracingPipeline {
                handle,
                owner: Arc::downgrade(&self),
                index,
                group_handlers: bytes,
            },
            info,
        ))
    }

    fn create_ray_tracing_shader_binding_table(
        self: Arc<Self>,
        pipeline: &RayTracingPipeline,
        info: ShaderBindingTableInfo<'_>,
    ) -> Result<ShaderBindingTable, OutOfMemory> {
        let group_size = u64::from(self.properties.rt.shader_group_handle_size);
        let group_align =
            u64::from(self.properties.rt.shader_group_base_alignment - 1);

        let group_count_usize = info.raygen.is_some() as usize
            + info.miss.len()
            + info.hit.len()
            + info.callable.len();

        let group_count =
            u32::try_from(group_count_usize).map_err(|_| OutOfMemory)?;

        let total_size = (group_size.checked_mul(u64::from(group_count)))
            .ok_or(OutOfMemory)?;

        let total_size_usize = usize::try_from(total_size)
            .unwrap_or_else(|_| out_of_host_memory());

        let mut bytes = vec![0; total_size_usize];

        let mut write_offset = 0;

        let group_handlers = &pipeline.erupt_ref(&*self).group_handlers;

        let raygen_handlers = copy_group_handlers(
            group_handlers,
            &mut bytes,
            info.raygen.iter().copied(),
            &mut write_offset,
            group_size,
        );

        let miss_handlers = copy_group_handlers(
            group_handlers,
            &mut bytes,
            info.miss.iter().copied(),
            &mut write_offset,
            group_size,
        );

        let hit_handlers = copy_group_handlers(
            group_handlers,
            &mut bytes,
            info.hit.iter().copied(),
            &mut write_offset,
            group_size,
        );

        let callable_handlers = copy_group_handlers(
            group_handlers,
            &mut bytes,
            info.callable.iter().copied(),
            &mut write_offset,
            group_size,
        );

        let buffer = self.create_buffer_static(
            BufferInfo {
                align: group_align,
                size: total_size,
                usage: BufferUsage::RAY_TRACING,
                memory: MemoryUsageFlags::UPLOAD,
            },
            &bytes,
        )?;

        Ok(ShaderBindingTable {
            raygen: raygen_handlers.map(|range| StridedBufferRegion {
                buffer: buffer.clone(),
                offset: range.start,
                size: range.end - range.start,
                stride: group_size,
            }),

            miss: miss_handlers.map(|range| StridedBufferRegion {
                buffer: buffer.clone(),
                offset: range.start,
                size: range.end - range.start,
                stride: group_size,
            }),

            hit: hit_handlers.map(|range| StridedBufferRegion {
                buffer: buffer.clone(),
                offset: range.start,
                size: range.end - range.start,
                stride: group_size,
            }),

            callable: callable_handlers.map(|range| StridedBufferRegion {
                buffer: buffer.clone(),
                offset: range.start,
                size: range.end - range.start,
                stride: group_size,
            }),
        })
    }

    fn create_descriptor_set_layout(
        self: Arc<Self>,
        info: DescriptorSetLayoutInfo,
    ) -> Result<DescriptorSetLayout, OutOfMemory> {
        let handle = if make_version(1, 2, 0) > self.graphics.version {
            assert!(
                info.bindings.iter().all(|binding| binding.flags.is_empty()),
                "Vulkan 1.2 is required for non-empty `DescriptorBindingFlags`",
            );

            unsafe {
                self.logical.create_descriptor_set_layout(
                    &vk1_0::DescriptorSetLayoutCreateInfo::default()
                        .builder()
                        .bindings(
                            &info
                                .bindings
                                .iter()
                                .map(|binding| {
                                    vk1_0::DescriptorSetLayoutBinding::default()
                                        .builder()
                                        .binding(binding.binding)
                                        .descriptor_count(binding.count)
                                        .descriptor_type(binding.ty.to_erupt())
                                        .stage_flags(binding.stages.to_erupt())
                                })
                                .collect::<SmallVec<[_; 16]>>(),
                        )
                        .flags(info.flags.to_erupt()),
                    None,
                    None,
                )
            }
        } else {
            let flags = info
                .bindings
                .iter()
                .map(|binding| binding.flags.to_erupt())
                .collect::<SmallVec<[_; 16]>>();

            unsafe {
                let bindings = info
                    .bindings
                    .iter()
                    .map(|binding| {
                        vk1_0::DescriptorSetLayoutBinding::default()
                            .builder()
                            .binding(binding.binding)
                            .descriptor_count(binding.count)
                            .descriptor_type(binding.ty.to_erupt())
                            .stage_flags(binding.stages.to_erupt())
                    })
                    .collect::<SmallVec<[_; 16]>>();
                let mut create_info =
                    vk1_0::DescriptorSetLayoutCreateInfo::default()
                        .builder()
                        .bindings(&bindings)
                        .flags(info.flags.to_erupt());

                let mut flags =
                    vk1_2::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                        .builder()
                        .binding_flags(&flags);

                flags.extend(&mut *create_info);

                self.logical.create_descriptor_set_layout(
                    &create_info,
                    None,
                    None,
                )
            }
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.descriptor_set_layouts.lock().insert(handle);

        let sizes = DescriptorSizes::from_bindings(&info.bindings);

        Ok(DescriptorSetLayout::make(
            EruptDescriptorSetLayout {
                handle,
                owner: Arc::downgrade(&self),
                index,
                sizes,
            },
            info,
        ))
    }

    fn create_descriptor_set(
        self: Arc<Self>,
        info: DescriptorSetInfo,
    ) -> Result<DescriptorSet, OutOfMemory> {
        let layout = info.layout.erupt_ref(&*self);
        let mut pool_flags = vk1_0::DescriptorPoolCreateFlags::empty();

        if info
            .layout
            .info()
            .flags
            .contains(DescriptorSetLayoutFlags::UPDATE_AFTER_BIND_POOL)
        {
            pool_flags |= vk1_0::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND;
        }

        let pool = unsafe {
            self.logical.create_descriptor_pool(
                &vk1_0::DescriptorPoolCreateInfo::default()
                    .builder()
                    .max_sets(1)
                    .pool_sizes(&layout.sizes)
                    .flags(pool_flags),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let handles = unsafe {
            self.logical.allocate_descriptor_sets(
                &vk1_0::DescriptorSetAllocateInfo::default()
                    .builder()
                    .descriptor_pool(pool)
                    .set_layouts(&[layout.handle]),
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        debug_assert_eq!(handles.len(), 1);

        let handle = handles[0];

        let index = self.descriptor_sets.lock().insert(handle);

        let pool_index = self.descriptor_pools.lock().insert(pool);

        Ok(DescriptorSet::make(
            EruptDescriptorSet {
                handle,
                pool,
                owner: Arc::downgrade(&self),
                index,
                pool_index,
            },
            info,
        ))
    }

    fn update_descriptor_sets(
        &self,
        writes: &[WriteDescriptorSet<'_>],
        copies: &[CopyDescriptorSet<'_>],
    ) {
        assert!(copies.is_empty(), "Unimplemented");

        let mut ranges = SmallVec::<[_; 64]>::new();

        let mut images = SmallVec::<[_; 16]>::new();

        let mut buffers = SmallVec::<[_; 16]>::new();

        // let mut buffer_views = SmallVec::<[_; 16]
        let mut acceleration_structures = SmallVec::<[_; 64]>::new();

        let mut write_descriptor_acceleration_structures =
            SmallVec::<[_; 16]>::new();

        for write in writes {
            match write.descriptors {
                Descriptors::Sampler(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(|sampler| {
                        vk1_0::DescriptorImageInfo::default()
                            .builder()
                            .sampler(sampler.erupt_ref(self).handle)
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::SampledImage(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(|(view, layout)| {
                        vk1_0::DescriptorImageInfo::default()
                            .builder()
                            .image_view(view.erupt_ref(self).handle)
                            .image_layout(layout.to_erupt())
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::CombinedImageSampler(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(
                        |(view, layout, sampler)| {
                            vk1_0::DescriptorImageInfo::default()
                                .builder()
                                .sampler(sampler.erupt_ref(self).handle)
                                .image_view(view.erupt_ref(self).handle)
                                .image_layout(layout.to_erupt())
                        },
                    ));

                    ranges.push(start..images.len());
                }
                Descriptors::StorageImage(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(|(view, layout)| {
                        vk1_0::DescriptorImageInfo::default()
                            .builder()
                            .image_view(view.erupt_ref(self).handle)
                            .image_layout(layout.to_erupt())
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::UniformBuffer(slice) => {
                    let start = buffers.len();

                    buffers.extend(slice.iter().map(
                        |(buffer, offset, size)| {
                            vk1_0::DescriptorBufferInfo::default()
                                .builder()
                                .buffer(buffer.erupt_ref(self).handle)
                                .offset(*offset)
                                .range(*size)
                        },
                    ));

                    ranges.push(start..buffers.len());
                }
                Descriptors::StorageBuffer(slice) => {
                    let start = buffers.len();

                    buffers.extend(slice.iter().map(
                        |(buffer, offset, size)| {
                            vk1_0::DescriptorBufferInfo::default()
                                .builder()
                                .buffer(buffer.erupt_ref(self).handle)
                                .offset(*offset)
                                .range(*size)
                        },
                    ));

                    ranges.push(start..buffers.len());
                }
                Descriptors::UniformBufferDynamic(slice) => {
                    let start = buffers.len();

                    buffers.extend(slice.iter().map(
                        |(buffer, offset, size)| {
                            vk1_0::DescriptorBufferInfo::default()
                                .builder()
                                .buffer(buffer.erupt_ref(self).handle)
                                .offset(*offset)
                                .range(*size)
                        },
                    ));

                    ranges.push(start..buffers.len());
                }
                Descriptors::StorageBufferDynamic(slice) => {
                    let start = buffers.len();

                    buffers.extend(slice.iter().map(
                        |(buffer, offset, size)| {
                            vk1_0::DescriptorBufferInfo::default()
                                .builder()
                                .buffer(buffer.erupt_ref(self).handle)
                                .offset(*offset)
                                .range(*size)
                        },
                    ));

                    ranges.push(start..buffers.len());
                }
                Descriptors::InputAttachment(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(|(view, layout)| {
                        vk1_0::DescriptorImageInfo::default()
                            .builder()
                            .image_view(view.erupt_ref(self).handle)
                            .image_layout(layout.to_erupt())
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::AccelerationStructure(slice) => {
                    let start = acceleration_structures.len();

                    acceleration_structures.extend(
                        slice.iter().map(|accs| accs.erupt_ref(self).handle),
                    );

                    ranges.push(start..acceleration_structures.len());

                    write_descriptor_acceleration_structures.push(
                        vkrt::WriteDescriptorSetAccelerationStructureKHR::default().builder(),
                    );
                }
            }
        }

        let mut ranges = ranges.into_iter();

        let mut write_descriptor_acceleration_structures =
            write_descriptor_acceleration_structures.iter_mut();

        let writes: SmallVec<[_; 16]> = writes
            .iter()
            .map(|write| {
                let builder = vk1_0::WriteDescriptorSet::default()
                    .builder()
                    .dst_set(write.set.erupt_ref(self).handle)
                    .dst_binding(write.binding)
                    .dst_array_element(write.element);

                match write.descriptors {
                    Descriptors::Sampler(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::SAMPLER)
                        .image_info(&images[ranges.next().unwrap()]),
                    Descriptors::CombinedImageSampler(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&images[ranges.next().unwrap()]),
                    Descriptors::SampledImage(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::SAMPLED_IMAGE)
                        .image_info(&images[ranges.next().unwrap()]),
                    Descriptors::StorageImage(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::STORAGE_IMAGE)
                        .image_info(&images[ranges.next().unwrap()]),
                    // Descriptors::UniformTexelBuffer(_) => todo!(),
                    // Descriptors::StorageTexelBuffer(_) => todo!(),
                    Descriptors::UniformBuffer(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&buffers[ranges.next().unwrap()]),
                    Descriptors::StorageBuffer(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(&buffers[ranges.next().unwrap()]),
                    Descriptors::UniformBufferDynamic(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                        .buffer_info(&buffers[ranges.next().unwrap()]),
                    Descriptors::StorageBufferDynamic(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::STORAGE_BUFFER_DYNAMIC)
                        .buffer_info(&buffers[ranges.next().unwrap()]),
                    Descriptors::InputAttachment(_) => builder
                        .descriptor_type(vk1_0::DescriptorType::INPUT_ATTACHMENT)
                        .image_info(&images[ranges.next().unwrap()]),
                    Descriptors::AccelerationStructure(_) => {
                        let range = ranges.next().unwrap();
                        let mut write = builder
                            .descriptor_type(vk1_0::DescriptorType::ACCELERATION_STRUCTURE_KHR);
                        write.descriptor_count = range.len() as u32;

                        let acc_structure_write =
                            write_descriptor_acceleration_structures.next().unwrap();

                        *acc_structure_write =
                            vkrt::WriteDescriptorSetAccelerationStructureKHR::default()
                                .builder()
                                .acceleration_structures(&acceleration_structures[range.clone()]);
                        unsafe { acc_structure_write.extend(&mut *write) };

                        write
                    }
                }
            })
            .collect();

        unsafe { self.logical.update_descriptor_sets(&writes, &[]) }
    }

    fn create_sampler(
        self: Arc<Self>,
        info: SamplerInfo,
    ) -> Result<Sampler, OutOfMemory> {
        let handle = unsafe {
            self.logical.create_sampler(
                &vk1_0::SamplerCreateInfo::default()
                    .builder()
                    .mag_filter(info.mag_filter.to_erupt())
                    .min_filter(info.min_filter.to_erupt())
                    .mipmap_mode(info.mipmap_mode.to_erupt())
                    .address_mode_u(info.address_mode_u.to_erupt())
                    .address_mode_v(info.address_mode_v.to_erupt())
                    .address_mode_w(info.address_mode_w.to_erupt())
                    .mip_lod_bias(info.mip_lod_bias.into_inner())
                    .anisotropy_enable(info.max_anisotropy.is_some())
                    .max_anisotropy(
                        info.max_anisotropy.unwrap_or(0.0.into()).into_inner(),
                    )
                    .compare_enable(info.compare_op.is_some())
                    .compare_op(match info.compare_op {
                        Some(compare_op) => compare_op.to_erupt(),
                        None => vk1_0::CompareOp::NEVER,
                    })
                    .min_lod(info.min_lod.into_inner())
                    .max_lod(info.max_lod.into_inner())
                    .border_color(info.border_color.to_erupt())
                    .unnormalized_coordinates(info.unnormalized_coordinates),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.samplers.lock().insert(handle);
        Ok(Sampler::make(
            EruptSampler {
                handle,
                owner: Arc::downgrade(&self),
                index,
            },
            info,
        ))
    }

    fn map_memory(
        &self,
        buffer: &Buffer,
        offset: u64,
        size: usize,
    ) -> &mut [MaybeUninit<u8>] {
        // FIXME: Map only subrange of the block.
        // FIXME: Ensure block isn't mapped.

        let block = &buffer.erupt_ref(&*self).block;

        unsafe {
            let ptr = match block.map(&self.logical, offset, size) {
                Ok(ptr) => ptr,
                Err(err) => {
                    panic!("Failed to map memory block {:#?}: {}", block, err,);
                }
            };
            std::slice::from_raw_parts_mut(ptr.as_ptr() as _, size)
        }
    }

    fn unmap_memory(&self, buffer: &Buffer) {
        let block = &buffer.erupt_ref(&*self).block;
        unsafe { block.unmap(&self.logical) }
    }
}

pub(super) fn create_render_pass_error_from_erupt(
    err: vk1_0::Result,
) -> CreateRenderPassError {
    match err {
        vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => out_of_host_memory(),
        vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            CreateRenderPassError::OutOfMemory {
                source: OutOfMemory,
            }
        }
        _ => CreateRenderPassError::Other {
            source: Box::new(err),
        },
    }
}

fn entry_name_to_cstr(name: &str) -> CString {
    CString::new(name.as_bytes())
        .expect("Shader names should not contain zero bytes")
}

fn copy_group_handlers(
    group_handlers: &[u8],
    write: &mut [u8],
    group_indices: impl IntoIterator<Item = u32>,
    write_offset: &mut usize,
    group_size: u64,
) -> Option<Range<u64>> {
    let result_start = u64::try_from(*write_offset).ok()?;
    let group_size_usize = usize::try_from(group_size).ok()?;

    for group_index in group_indices {
        let group_offset =
            (group_size_usize.checked_mul(usize::try_from(group_index).ok()?))?;

        let group_end = group_offset.checked_add(group_size_usize)?;
        let write_end = write_offset.checked_add(group_size_usize)?;

        let group_range = group_offset..group_end;
        let write_range = *write_offset..write_end;

        let handler = &group_handlers[group_range];
        let output = &mut write[write_range];

        output.copy_from_slice(handler);
        *write_offset = write_end;
    }

    let result_end = u64::try_from(*write_offset).ok()?;
    Some(result_start..result_end)
}

#[cfg(feature = "shader-compiler")]
mod shader_compiler {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum ShaderCompileFailed {
        #[error("Failed to compile shader. UTF-8 shader source code expected: {source}")]
        NonUTF8 {
            #[from]
            source: std::str::Utf8Error,
        },

        #[error("Shaderc failed to compile shader source code: {source}")]
        Shaderc {
            #[from]
            source: shaderc::Error,
        },

        #[error("Unsupported shader language {language}")]
        Unsupported { language: ShaderLanguage },
    }

    pub fn compile_shader(
        code: &[u8],
        entry: &str,
        language: ShaderLanguage,
    ) -> Result<Box<[u8]>, ShaderCompileFailed> {
        let mut options = shaderc::CompileOptions::new().unwrap();

        options.set_source_language(match language {
            ShaderLanguage::GLSL => shaderc::SourceLanguage::GLSL,
            ShaderLanguage::HLSL => shaderc::SourceLanguage::HLSL,
            ShaderLanguage::SPIRV => return Ok(code.into()),
            _ => return Err(ShaderCompileFailed::Unsupported { language }),
        });

        let mut compiler = shaderc::Compiler::new().unwrap();

        let binary_result = compiler.compile_into_spirv(
            std::str::from_utf8(code)?,
            shaderc::ShaderKind::InferFromSource,
            match language {
                ShaderLanguage::GLSL => "shader.glsl",
                ShaderLanguage::HLSL => "shader.hlsl",
                ShaderLanguage::SPIRV => return Ok(code.into()), // You again?!
                _ => return Err(ShaderCompileFailed::Unsupported { language }),
            },
            entry,
            Some(&options),
        )?;

        if !binary_result.get_warning_messages().is_empty() {
            tracing::warn!("{}", binary_result.get_warning_messages());
        }

        Ok(binary_result.as_binary_u8().into())
    }
}

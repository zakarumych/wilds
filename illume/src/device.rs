use crate::{
    accel::{
        AccelerationStructure, AccelerationStructureInfo,
        AccelerationStructureLevel,
    },
    access::supported_access,
    arith_eq, arith_le, arith_ne, assert_object,
    buffer::{Buffer, BufferInfo, BufferUsage, StridedBufferRegion},
    convert::{
        from_erupt, memory_usage_to_tvma, oom_error_from_erupt, ToErupt as _,
    },
    descriptor::{
        CopyDescriptorSet, DescriptorSet, DescriptorSetInfo,
        DescriptorSetLayout, DescriptorSetLayoutFlags, DescriptorSetLayoutInfo,
        DescriptorSizes, Descriptors, WriteDescriptorSet,
    },
    fence::{Fence, FenceInfo},
    framebuffer::{Framebuffer, FramebufferInfo},
    graphics::Graphics,
    host_memory_space_overlow,
    image::{Image, ImageInfo},
    memory::MemoryUsageFlags,
    out_of_host_memory,
    physical::{Features, Properties},
    pipeline::{
        ColorBlend, GraphicsPipeline, GraphicsPipelineInfo, PipelineLayout,
        PipelineLayoutInfo, RayTracingPipeline, RayTracingPipelineInfo,
        RayTracingShaderGroupInfo, ShaderBindingTable, ShaderBindingTableInfo,
        State,
    },
    render_pass::{RenderPass, RenderPassInfo},
    sampler::{Sampler, SamplerInfo},
    semaphore::{Semaphore, SemaphoreInfo},
    shader::{
        CreateShaderModuleError, InvalidShader, ShaderLanguage, ShaderModule,
        ShaderModuleInfo,
    },
    surface::{Surface, SurfaceError, Swapchain},
    view::{ImageView, ImageViewInfo, ImageViewKind},
    DeviceAddress, OutOfMemory,
};
use bumpalo::{collections::Vec as BVec, Bump};
use bytemuck::Pod;
use smallvec::SmallVec;
use std::{
    convert::{TryFrom as _, TryInto as _},
    error::Error,
    ffi::CString,
    fmt::{self, Debug},
    mem::{size_of_val, MaybeUninit},
    ops::{Deref, Range},
    sync::{Arc, Weak},
};

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

use parking_lot::Mutex;
use slab::Slab;

#[derive(Debug, thiserror::Error)]
pub enum CreateDeviceError<E: Error + 'static> {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Non-existed families are requested")]
    BadFamiliesRequested,

    #[error("{source}")]
    CannotFindRequeredQueues { source: E },

    /// Implementation specific error.
    #[error("Failed to load core functions")]
    CoreFunctionLoadFailed,

    #[error("Failed to load advertized extension ({extension}) functions")]
    ExtensionLoadFailed { extension: &'static str },

    #[error("Function returned unexpected error code: {result}")]
    UnexpectedVulkanResult { result: vk1_0::Result },
}

/// Possible error which can be returned from `create_buffer_*`.
#[derive(Debug, thiserror::Error)]
pub enum CreateBufferError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Buffer usage {usage:?} is unsupported")]
    UnsupportedUsage { usage: BufferUsage },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
}

/// Possible error which can be returned from `create_image_*)`.
#[derive(Debug, thiserror::Error)]
pub enum CreateImageError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Combination paramters `{info:?}` is unsupported")]
    Unsupported { info: ImageInfo },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
}

pub(crate) struct Inner {
    logical: DeviceLoader,
    physical: vk1_0::PhysicalDevice,
    properties: Properties,
    features: Features,
    allocator: tvma::Allocator,
    version: u32,
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
    swapchains: Mutex<Slab<vksw::SwapchainKHR>>,
}

impl Debug for Inner {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("Device")
                .field("logical", &self.logical.handle)
                .field("physical", &self.physical)
                .finish()
        } else {
            Debug::fmt(&self.logical.handle, fmt)
        }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct WeakDevice {
    inner: Weak<Inner>,
}

impl Debug for WeakDevice {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.upgrade() {
            Some(device) => device.fmt(fmt),
            None => write!(fmt, "Destroyed device: {:p}", self.inner.as_ptr()),
        }
    }
}

impl WeakDevice {
    pub fn upgrade(&self) -> Option<Device> {
        self.inner.upgrade().map(|inner| Device { inner })
    }

    pub fn is(&self, device: &Device) -> bool {
        self.inner.as_ptr() == &*device.inner
    }
}

/// Opaque value that represents graphics API device.
/// It is used to manage (create, destroy, check state) most of the device
/// resources.
#[derive(Clone)]
#[repr(transparent)]
pub struct Device {
    inner: Arc<Inner>,
}

impl Debug for Device {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("Device")
                .field("logical", &self.inner.logical.handle)
                .field("physical", &self.inner.physical)
                .finish()
        } else {
            Debug::fmt(&self.inner.logical.handle, fmt)
        }
    }
}

impl Device {
    pub(crate) fn logical(&self) -> &DeviceLoader {
        &self.inner.logical
    }

    pub(crate) fn physical(&self) -> vk1_0::PhysicalDevice {
        self.inner.physical
    }

    pub(crate) fn properties(&self) -> &Properties {
        &self.inner.properties
    }

    pub(crate) fn features(&self) -> &Features {
        &self.inner.features
    }

    pub(crate) fn allocator(&self) -> &tvma::Allocator {
        &self.inner.allocator
    }

    pub(crate) fn version(&self) -> u32 {
        self.inner.version
    }

    pub(crate) fn buffers(&self) -> &Mutex<Slab<vk1_0::Buffer>> {
        &self.inner.buffers
    }

    // pub(crate) fn buffer_views(&self) -> &Mutex<Slab<vk1_0::BufferView>> {
    //     &self.inner.buffer_views
    // }

    pub(crate) fn descriptor_pools(
        &self,
    ) -> &Mutex<Slab<vk1_0::DescriptorPool>> {
        &self.inner.descriptor_pools
    }

    pub(crate) fn descriptor_sets(&self) -> &Mutex<Slab<vk1_0::DescriptorSet>> {
        &self.inner.descriptor_sets
    }

    pub(crate) fn descriptor_set_layouts(
        &self,
    ) -> &Mutex<Slab<vk1_0::DescriptorSetLayout>> {
        &self.inner.descriptor_set_layouts
    }

    pub(crate) fn fences(&self) -> &Mutex<Slab<vk1_0::Fence>> {
        &self.inner.fences
    }

    pub(crate) fn framebuffers(&self) -> &Mutex<Slab<vk1_0::Framebuffer>> {
        &self.inner.framebuffers
    }

    pub(crate) fn images(&self) -> &Mutex<Slab<vk1_0::Image>> {
        &self.inner.images
    }

    pub(crate) fn image_views(&self) -> &Mutex<Slab<vk1_0::ImageView>> {
        &self.inner.image_views
    }

    pub(crate) fn pipelines(&self) -> &Mutex<Slab<vk1_0::Pipeline>> {
        &self.inner.pipelines
    }

    pub(crate) fn pipeline_layouts(
        &self,
    ) -> &Mutex<Slab<vk1_0::PipelineLayout>> {
        &self.inner.pipeline_layouts
    }

    pub(crate) fn render_passes(&self) -> &Mutex<Slab<vk1_0::RenderPass>> {
        &self.inner.render_passes
    }

    pub(crate) fn semaphores(&self) -> &Mutex<Slab<vk1_0::Semaphore>> {
        &self.inner.semaphores
    }

    pub(crate) fn shaders(&self) -> &Mutex<Slab<vk1_0::ShaderModule>> {
        &self.inner.shaders
    }

    pub(crate) fn acceleration_strucutres(
        &self,
    ) -> &Mutex<Slab<vkrt::AccelerationStructureKHR>> {
        &self.inner.acceleration_strucutres
    }

    pub(crate) fn samplers(&self) -> &Mutex<Slab<vk1_0::Sampler>> {
        &self.inner.samplers
    }

    pub(crate) fn swapchains(&self) -> &Mutex<Slab<vksw::SwapchainKHR>> {
        &self.inner.swapchains
    }

    pub(crate) fn new(
        logical: DeviceLoader,
        physical: vk1_0::PhysicalDevice,
        properties: Properties,
        features: Features,
        version: u32,
    ) -> Self {
        Device {
            inner: Arc::new(Inner {
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
                version,
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
            }),
        }
    }

    pub(crate) fn graphics(&self) -> &'static Graphics {
        unsafe {
            // Device can be created only via Graphics instance.
            Graphics::get_unchecked()
        }
    }

    pub(crate) fn downgrade(&self) -> WeakDevice {
        WeakDevice {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Creates buffer with uninitialized content.
    #[tracing::instrument]
    pub fn create_buffer(
        &self,
        info: BufferInfo,
    ) -> Result<Buffer, OutOfMemory> {
        if info.usage.contains(BufferUsage::SHADER_DEVICE_ADDRESS) {
            assert_ne!(self.inner.features.v12.buffer_device_address, 0);
        }

        let handle = unsafe {
            self.inner.logical.create_buffer(
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
            self.inner
                .logical
                .get_buffer_memory_requirements(handle, None)
        };

        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.inner.allocator.alloc(
                &self.inner.logical,
                reqs.size,
                (reqs.alignment - 1) | info.align,
                reqs.memory_type_bits,
                memory_usage_to_tvma(info.memory),
                tvma::Dedicated::Indifferent,
            )
        }
        .map_err(|_| {
            unsafe { self.inner.logical.destroy_buffer(handle, None) }

            OutOfMemory
        })?;

        let result = unsafe {
            self.inner.logical.bind_buffer_memory(
                handle,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        if let Err(err) = result {
            unsafe {
                self.inner.logical.destroy_buffer(handle, None);

                self.inner.allocator.dealloc(&self.inner.logical, block);
            }

            return Err(oom_error_from_erupt(err));
        }

        let address = if info.usage.contains(BufferUsage::SHADER_DEVICE_ADDRESS)
        {
            Some(Option::unwrap(from_erupt(unsafe {
                self.inner.logical.get_buffer_device_address(
                    &vk1_2::BufferDeviceAddressInfo::default()
                        .builder()
                        .buffer(handle),
                )
            })))
        } else {
            None
        };

        let buffer_index = self.inner.buffers.lock().insert(handle);

        Ok(Buffer::make(
            info,
            handle,
            address,
            block,
            self.downgrade(),
            buffer_index,
        ))
    }

    /// Creates static buffer with preinitialized content from `data`.
    /// Implies `MemoryUsageFlags::Device`.
    ///
    /// # Panics
    ///
    /// Function will panic if creating buffer size does not equal data size.
    /// E.g. if `info.size != std::mem::size_of(data)`.
    #[tracing::instrument(skip(data))]
    pub fn create_buffer_static<T: 'static>(
        &self,
        info: BufferInfo,
        data: &[T],
    ) -> Result<Buffer, OutOfMemory>
    where
        T: Pod,
    {
        // tracing::error!("!");
        assert!(info.is_valid());
        if arith_ne(info.size, size_of_val(data)) {
            panic!(
                "Buffer size {} does not match data size {}",
                info.size,
                data.len()
            );
        }

        debug_assert!(arith_eq(info.size, data.len()));
        assert!(info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD
        ));

        let buffer = self.create_buffer(info)?;

        unsafe {
            match buffer.block(self).map(&self.inner.logical, 0, data.len()) {
                Ok(ptr) => {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr() as *const u8,
                        ptr.as_ptr(),
                        data.len(),
                    );

                    buffer.block(self).unmap(&self.inner.logical);

                    Ok(buffer)
                }
                Err(tvma::MappingError::OutOfMemory { .. }) => Err(OutOfMemory),
                Err(tvma::MappingError::NonHostVisible)
                | Err(tvma::MappingError::OutOfBounds) => unreachable!(),
            }
        }
    }

    /// Creates a fence.
    /// Fences are create in unsignaled state.
    #[tracing::instrument]
    pub fn create_fence(&self) -> Result<Fence, OutOfMemory> {
        let fence = unsafe {
            self.inner.logical.create_fence(
                &vk1_0::FenceCreateInfo::default().builder(),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.inner.fences.lock().insert(fence);

        Ok(Fence::make(FenceInfo, fence, self.downgrade(), index))
    }

    /// Creates framebuffer for specified render pass from views.
    #[tracing::instrument]
    pub fn create_framebuffer(
        &self,
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
            .map(|view| view.handle(&*self))
            .collect::<SmallVec<[_; 16]>>();

        let framebuffer = unsafe {
            self.inner.logical.create_framebuffer(
                &vk1_0::FramebufferCreateInfo::default()
                    .builder()
                    .render_pass(info.render_pass.handle(&*self))
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

        let index = self.inner.framebuffers.lock().insert(framebuffer);

        Ok(Framebuffer::make(
            info,
            framebuffer,
            self.downgrade(),
            index,
        ))
    }

    /// Creates graphics pipeline.
    #[tracing::instrument]
    pub fn create_graphics_pipeline(
        &self,
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
                .module(info.vertex_shader.module().handle(&*self))
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
                    dynamic_states.push(vk1_0::DynamicState::VIEWPORT);
                    builder = builder.viewport_count(1);
                }
            }

            match &rasterizer.scissor {
                State::Static { value } => {
                    scissor = value.to_erupt().builder();

                    builder = builder.scissors(std::slice::from_ref(&scissor));
                }
                State::Dynamic => {
                    dynamic_states.push(vk1_0::DynamicState::SCISSOR);
                    builder = builder.scissor_count(1);
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
                        .module(shader.module().handle(&*self))
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
            .layout(info.layout.handle(&*self))
            .render_pass(info.render_pass.handle(&*self))
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
            self.inner.logical.create_graphics_pipelines(
                vk1_0::PipelineCache::null(),
                &[builder],
                None,
            )
        }
        .result()
        .map_err(|err| oom_error_from_erupt(err))?;

        debug_assert_eq!(pipelines.len(), 1);

        let pipeline = pipelines[0];

        let index = self.inner.pipelines.lock().insert(pipeline);

        drop(shader_stages);

        Ok(GraphicsPipeline::make(
            info,
            pipeline,
            self.downgrade(),
            index,
        ))
    }

    /// Creates image with uninitialized content.
    #[tracing::instrument]
    pub fn create_image(
        &self,
        info: ImageInfo,
    ) -> Result<Image, CreateImageError> {
        let image = unsafe {
            self.inner.logical.create_image(
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

        let reqs = unsafe {
            self.inner
                .logical
                .get_image_memory_requirements(image, None)
        };

        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.inner
                .allocator
                .alloc(
                    &self.inner.logical,
                    reqs.size,
                    reqs.alignment - 1,
                    reqs.memory_type_bits,
                    memory_usage_to_tvma(info.memory),
                    tvma::Dedicated::Indifferent,
                )
                .map_err(|_| {
                    self.inner.logical.destroy_image(image, None);

                    OutOfMemory
                })
        }?;

        let result = unsafe {
            self.inner.logical.bind_image_memory(
                image,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        match result {
            Ok(()) => {
                let index = self.inner.images.lock().insert(image);

                Ok(Image::make(
                    info,
                    image,
                    Some(block),
                    self.downgrade(),
                    index,
                ))
            }
            Err(err) => {
                unsafe {
                    self.inner.logical.destroy_image(image, None);
                    self.inner.allocator.dealloc(&self.inner.logical, block);
                }

                Err(oom_error_from_erupt(err).into())
            }
        }
    }

    /// Creates static image with preinitialized content from `data`.
    ///
    /// # Panics
    ///
    /// Function will panic if creating image size does not equal data size.
    #[tracing::instrument(skip(data))]
    pub fn create_image_static<T>(
        &self,
        info: ImageInfo,
        data: &[T],
    ) -> Result<Image, CreateImageError>
    where
        T: Pod,
    {
        assert!(info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD
        ));

        let image = unsafe {
            self.inner.logical.create_image(
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

        let reqs = unsafe {
            self.inner
                .logical
                .get_image_memory_requirements(image, None)
        };

        debug_assert!(arith_eq(reqs.size, data.len()));
        debug_assert!(reqs.alignment.is_power_of_two());

        let block = unsafe {
            self.inner
                .allocator
                .alloc(
                    &self.inner.logical,
                    reqs.size,
                    reqs.alignment - 1,
                    reqs.memory_type_bits,
                    memory_usage_to_tvma(info.memory),
                    tvma::Dedicated::Indifferent,
                )
                .map_err(|_| {
                    self.inner.logical.destroy_image(image, None);

                    OutOfMemory
                })
        }?;

        let result = unsafe {
            self.inner.logical.bind_image_memory(
                image,
                block.memory(),
                block.offset(),
            )
        }
        .result();

        if let Err(err) = result {
            unsafe {
                self.inner.logical.destroy_image(image, None);
                self.inner.allocator.dealloc(&self.inner.logical, block);
            }
            return Err(oom_error_from_erupt(err).into());
        }

        unsafe {
            match block.map(&self.inner.logical, 0, data.len()) {
                Ok(ptr) => {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr() as *const u8,
                        ptr.as_ptr(),
                        data.len(),
                    );

                    block.unmap(&self.inner.logical);
                }
                Err(tvma::MappingError::OutOfMemory { .. }) => {
                    return Err(OutOfMemory.into());
                }
                Err(tvma::MappingError::NonHostVisible)
                | Err(tvma::MappingError::OutOfBounds) => unreachable!(),
            }
        }

        let index = self.inner.images.lock().insert(image);

        Ok(Image::make(
            info,
            image,
            Some(block),
            self.downgrade(),
            index,
        ))
    }

    /// Creates view to an image.
    #[tracing::instrument]
    pub fn create_image_view(
        &self,
        info: ImageViewInfo,
    ) -> Result<ImageView, OutOfMemory> {
        let image = &info.image;

        let view = unsafe {
            self.inner.logical.create_image_view(
                &vk1_0::ImageViewCreateInfo::default()
                    .builder()
                    .image(image.handle(self))
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

        let index = self.inner.image_views.lock().insert(view);

        Ok(ImageView::make(info, view, self.downgrade(), index))
    }

    /// Creates pipeline layout.
    #[tracing::instrument]
    pub fn create_pipeline_layout(
        &self,
        info: PipelineLayoutInfo,
    ) -> Result<PipelineLayout, OutOfMemory> {
        let pipeline_layout = unsafe {
            self.inner.logical.create_pipeline_layout(
                &vk1_0::PipelineLayoutCreateInfo::default()
                    .builder()
                    .set_layouts(
                        &info
                            .sets
                            .iter()
                            .map(|set| set.handle(self))
                            .collect::<SmallVec<[_; 16]>>(),
                    ),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.inner.pipeline_layouts.lock().insert(pipeline_layout);

        Ok(PipelineLayout::make(
            info,
            pipeline_layout,
            self.downgrade(),
            index,
        ))
    }

    /// Creates render pass.
    #[tracing::instrument]
    pub fn create_render_pass(
        &self,
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
            .collect::<Vec<_>>();

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
            self.inner.logical.create_render_pass(
                &render_passs_create_info,
                None,
                None,
            )
        }
        .result()
        .map_err(create_render_pass_error_from_erupt)?;

        let index = self.inner.render_passes.lock().insert(render_pass);

        Ok(RenderPass::make(info, render_pass, self.downgrade(), index))
    }

    pub(crate) fn create_semaphore_raw(
        &self,
    ) -> Result<(vk1_0::Semaphore, usize), vk1_0::Result> {
        let semaphore = unsafe {
            self.inner.logical.create_semaphore(
                &vk1_0::SemaphoreCreateInfo::default().builder(),
                None,
                None,
            )
        }
        .result()?;

        let index = self.inner.semaphores.lock().insert(semaphore);

        Ok((semaphore, index))
    }

    /// Creates semaphore. Semaphores are created in unsignaled state.
    #[tracing::instrument]
    pub fn create_semaphore(&self) -> Result<Semaphore, OutOfMemory> {
        let (handle, index) =
            self.create_semaphore_raw().map_err(oom_error_from_erupt)?;

        Ok(Semaphore::make(
            SemaphoreInfo,
            handle,
            self.downgrade(),
            index,
        ))
    }

    #[tracing::instrument]
    pub fn create_shader_module(
        &self,
        info: ShaderModuleInfo,
    ) -> Result<ShaderModule, CreateShaderModuleError> {
        let code = match info.language {
            ShaderLanguage::SPIRV => &*info.code,
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
            self.inner.logical.create_shader_module(
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

        let index = self.inner.shaders.lock().insert(module);

        Ok(ShaderModule::make(info, module, self.downgrade(), index))
    }

    /// Creates swapchain for specified surface.
    /// Only one swapchain may be associated with one surface.
    #[tracing::instrument]
    pub fn create_swapchain(
        &self,
        surface: &mut Surface,
    ) -> Result<Swapchain, SurfaceError> {
        Ok(Swapchain::new(surface, self)?)
    }

    /// Resets fences.
    /// All specified fences must be in signalled state.
    /// Fences are moved into unsignalled state.
    #[tracing::instrument]
    pub fn reset_fences(&self, fences: &[&Fence]) {
        let fences = fences
            .iter()
            .map(|fence| fence.handle(self))
            .collect::<SmallVec<[_; 16]>>();

        unsafe { self.inner.logical.reset_fences(&fences) }
            .expect("TODO: Handle device lost")
    }

    #[tracing::instrument]
    pub fn is_fence_signalled(&self, fence: &Fence) -> bool {
        let fence = fence.handle(self);

        match unsafe { self.inner.logical.get_fence_status(fence) }.raw {
            vk1_0::Result::SUCCESS => true,
            vk1_0::Result::NOT_READY => true,
            vk1_0::Result::ERROR_DEVICE_LOST => panic!("Device lost"),
            err => panic!("Unexpected error: {}", err),
        }
    }

    /// Wait for fences to become signaled.
    /// If `all` is `true` - waits for all specified fences to become signaled.
    /// Otherwise waits for at least on of specified fences to become signaled.
    /// May return immediately if all fences are already signaled (or at least
    /// one is signaled if `all == false`). Fences are signaled by `Queue`s.
    /// See `Queue::submit`.
    #[tracing::instrument]
    pub fn wait_fences(&self, fences: &[&Fence], all: bool) {
        let fences = fences
            .iter()
            .map(|fence| fence.handle(self))
            .collect::<SmallVec<[_; 16]>>();

        unsafe { self.inner.logical.wait_for_fences(&fences, all, !0) }
            .expect("TODO: Handle device lost")
    }

    /// Wait for whole device to become idle. That is, wait for all pending
    /// operations to complete. This is equivalent to calling
    /// `Queue::wait_idle` for all queues. Typically used only before device
    /// destruction.
    #[tracing::instrument]
    pub fn wait_idle(&self) {
        unsafe {
            self.inner
                .logical
                .device_wait_idle()
                .expect("TODO: Handle device lost")
        }
    }

    /// Creates acceleration structure.
    ///
    /// # Panics
    ///
    /// This method may panic if `Feature::RayTracing` wasn't enabled.
    #[tracing::instrument]
    pub fn create_acceleration_structure(
        &self,
        info: AccelerationStructureInfo,
    ) -> Result<AccelerationStructure, OutOfMemory> {
        assert!(
            self.inner.logical.khr_ray_tracing.is_some(),
            "RayTracing feature is not enabled"
        );

        // assert_ne!(info.geometries.len(), 0);

        assert!(
            arith_le(
                info.geometries.len(),
                self.inner.properties.rt.max_geometry_count
            ),
            "Too many gemetries: {}. Limit: {}",
            info.geometries.len(),
            self.inner.properties.rt.max_geometry_count
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
            self.inner.logical.create_acceleration_structure_khr(
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
            self.inner.logical
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
            self.inner.allocator.alloc(
                &self.inner.logical,
                reqs.size,
                reqs.alignment - 1,
                reqs.memory_type_bits,
                tvma::UsageFlags::empty(),
                tvma::Dedicated::Indifferent,
            )
        }
        .map_err(|err| {
            unsafe {
                self.inner
                    .logical
                    .destroy_acceleration_structure_khr(handle, None);
            }
            tracing::error!("{}", err);
            OutOfMemory
        })?;

        let result = unsafe {
            self.inner.logical.bind_acceleration_structure_memory_khr(&[
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
                let index =
                    self.inner.acceleration_strucutres.lock().insert(handle);

                let address = Option::unwrap(from_erupt(unsafe {
                    self.inner.logical.get_acceleration_structure_device_address_khr(
                        &vkrt::AccelerationStructureDeviceAddressInfoKHR::default()
                            .builder()
                            .acceleration_structure(handle),
                    )
                }));

                Ok(AccelerationStructure::make(
                    info,
                    handle,
                    address,
                    block,
                    self.downgrade(),
                    index,
                ))
            }
            Err(err) => {
                unsafe {
                    self.inner
                        .logical
                        .destroy_acceleration_structure_khr(handle, None);
                    self.inner.allocator.dealloc(&self.inner.logical, block);
                }

                Err(oom_error_from_erupt(err).into())
            }
        }
    }

    /// Returns buffers device address.
    #[tracing::instrument]
    pub fn get_buffer_device_address(
        &self,
        buffer: &Buffer,
    ) -> Option<DeviceAddress> {
        if buffer
            .info()
            .usage
            .contains(BufferUsage::SHADER_DEVICE_ADDRESS)
        {
            assert_ne!(self.inner.features.v12.buffer_device_address, 0);

            Some(buffer.address(self).expect(
                "Device address for buffer must be set when `BufferUsage::SHADER_DEVICE_ADDRESS` is specified",
            ))
        } else {
            None
        }
    }

    #[tracing::instrument]
    pub fn get_acceleration_structure_device_address(
        &self,
        acceleration_structure: &AccelerationStructure,
    ) -> DeviceAddress {
        *acceleration_structure.address(self)
    }

    #[tracing::instrument]
    pub fn allocate_acceleration_structure_build_scratch(
        &self,
        acceleration_structure: &AccelerationStructure,
        update: bool,
    ) -> Result<Buffer, OutOfMemory> {
        assert!(
            self.inner.logical.khr_ray_tracing.is_some(),
            "RayTracing feature is not enabled"
        );

        // Collect memory requirements.
        let size = unsafe {
            self.inner.logical
                .get_acceleration_structure_memory_requirements_khr(
                    &vkrt::AccelerationStructureMemoryRequirementsInfoKHR::default()
                        .builder()
                        ._type(if update {
                            vkrt::AccelerationStructureMemoryRequirementsTypeKHR::UPDATE_SCRATCH_KHR
                        } else {
                            vkrt::AccelerationStructureMemoryRequirementsTypeKHR::BUILD_SCRATCH_KHR
                        })
                        .build_type(vkrt::AccelerationStructureBuildTypeKHR(1)) // TODO: Use assocated constant.
                        .acceleration_structure(acceleration_structure.handle(self)),
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

    #[tracing::instrument]
    pub fn create_ray_tracing_pipeline(
        &self,
        info: RayTracingPipelineInfo,
    ) -> Result<RayTracingPipeline, OutOfMemory> {
        assert!(
            self.inner.logical.khr_ray_tracing.is_some(),
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
                    .module(shader.module.handle(self))
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
            self.inner.logical.create_ray_tracing_pipelines_khr(
                vk1_0::PipelineCache::null(),
                &[vkrt::RayTracingPipelineCreateInfoKHR::default()
                    .builder()
                    .stages(&stages)
                    .groups(&groups)
                    .max_recursion_depth(info.max_recursion_depth)
                    .layout(info.layout.handle(self))],
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        assert_eq!(handles.len(), 1);

        let handle = handles[0];

        let group_size = self.inner.properties.rt.shader_group_handle_size;

        let group_size_usize =
            usize::try_from(group_size).map_err(|_| out_of_host_memory())?;

        let total_size_usize = group_size_usize
            .checked_mul(info.groups.len())
            .ok_or_else(host_memory_space_overlow)?;

        let group_count =
            u32::try_from(info.groups.len()).map_err(|_| OutOfMemory)?;

        let mut bytes = vec![0u8; total_size_usize];

        unsafe {
            self.inner.logical.get_ray_tracing_shader_group_handles_khr(
                handle,
                0,
                group_count,
                bytes.len(),
                bytes.as_mut_ptr() as *mut _,
            )
        }
        .result()
        .map_err(|err| {
            unsafe { self.inner.logical.destroy_pipeline(handle, None) }

            oom_error_from_erupt(err)
        })?;

        let index = self.inner.pipelines.lock().insert(handle);

        Ok(RayTracingPipeline::make(
            info,
            handle,
            bytes.into(),
            self.downgrade(),
            index,
        ))
    }

    #[tracing::instrument]
    pub fn create_descriptor_set_layout(
        &self,
        info: DescriptorSetLayoutInfo,
    ) -> Result<DescriptorSetLayout, OutOfMemory> {
        let handle = if make_version(1, 2, 0) > self.inner.version {
            assert!(
                info.bindings.iter().all(|binding| binding.flags.is_empty()),
                "Vulkan 1.2 is required for non-empty `DescriptorBindingFlags`",
            );

            unsafe {
                self.inner.logical.create_descriptor_set_layout(
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

                self.inner.logical.create_descriptor_set_layout(
                    &create_info,
                    None,
                    None,
                )
            }
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let index = self.inner.descriptor_set_layouts.lock().insert(handle);

        let sizes = DescriptorSizes::from_bindings(&info.bindings);

        Ok(DescriptorSetLayout::make(
            info,
            handle,
            sizes,
            self.downgrade(),
            index,
        ))
    }

    #[tracing::instrument]
    pub fn create_descriptor_set(
        &self,
        info: DescriptorSetInfo,
    ) -> Result<DescriptorSet, OutOfMemory> {
        let layout = &info.layout;
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
            self.inner.logical.create_descriptor_pool(
                &vk1_0::DescriptorPoolCreateInfo::default()
                    .builder()
                    .max_sets(1)
                    .pool_sizes(&layout.sizes(self))
                    .flags(pool_flags),
                None,
                None,
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        let handles = unsafe {
            self.inner.logical.allocate_descriptor_sets(
                &vk1_0::DescriptorSetAllocateInfo::default()
                    .builder()
                    .descriptor_pool(pool)
                    .set_layouts(&[layout.handle(self)]),
            )
        }
        .result()
        .map_err(oom_error_from_erupt)?;

        debug_assert_eq!(handles.len(), 1);

        let handle = handles[0];

        let index = self.inner.descriptor_sets.lock().insert(handle);
        let pool_index = self.inner.descriptor_pools.lock().insert(pool);

        Ok(DescriptorSet::make(
            info,
            handle,
            pool,
            pool_index,
            self.downgrade(),
            index,
        ))
    }

    #[tracing::instrument]
    pub fn update_descriptor_sets<'a>(
        &self,
        writes: &[WriteDescriptorSet<'a>],
        copies: &[CopyDescriptorSet<'a>],
    ) {
        for write in writes {
            write.validate();
        }

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
                            .sampler(sampler.handle(self))
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::SampledImage(slice) => {
                    let start = images.len();

                    images.extend(slice.iter().map(|(view, layout)| {
                        vk1_0::DescriptorImageInfo::default()
                            .builder()
                            .image_view(view.handle(self))
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
                                .sampler(sampler.handle(self))
                                .image_view(view.handle(self))
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
                            .image_view(view.handle(self))
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
                                .buffer(buffer.handle(self))
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
                                .buffer(buffer.handle(self))
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
                                .buffer(buffer.handle(self))
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
                                .buffer(buffer.handle(self))
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
                            .image_view(view.handle(self))
                            .image_layout(layout.to_erupt())
                    }));

                    ranges.push(start..images.len());
                }
                Descriptors::AccelerationStructure(slice) => {
                    let start = acceleration_structures.len();

                    acceleration_structures
                        .extend(slice.iter().map(|accs| accs.handle(self)));

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
                    .dst_set(write.set.handle(self))
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

        unsafe { self.inner.logical.update_descriptor_sets(&writes, &[]) }
    }

    #[tracing::instrument]
    pub fn create_sampler(
        &self,
        info: SamplerInfo,
    ) -> Result<Sampler, OutOfMemory> {
        let handle = unsafe {
            self.inner.logical.create_sampler(
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

        let index = self.inner.samplers.lock().insert(handle);
        Ok(Sampler::make(info, handle, self.downgrade(), index))
    }

    #[tracing::instrument]
    pub fn create_ray_tracing_shader_binding_table(
        &self,
        pipeline: &RayTracingPipeline,
        info: ShaderBindingTableInfo,
    ) -> Result<ShaderBindingTable, OutOfMemory> {
        let group_size =
            u64::from(self.inner.properties.rt.shader_group_handle_size);
        let group_align =
            u64::from(self.inner.properties.rt.shader_group_base_alignment - 1);

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

        let group_handlers = &pipeline.group_handlers(self);

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

    #[tracing::instrument]
    pub fn map_memory(
        &self,
        buffer: &Buffer,
        offset: u64,
        size: usize,
    ) -> &mut [MaybeUninit<u8>] {
        // FIXME: Track mapped blocks
        let block = buffer.block(self);

        unsafe {
            let ptr = match block.map(&self.inner.logical, offset, size) {
                Ok(ptr) => ptr,
                Err(err) => {
                    panic!("Failed to map memory block {:#?}: {}", block, err,);
                }
            };
            std::slice::from_raw_parts_mut(ptr.as_ptr() as _, size)
        }
    }

    fn unmap_memory(&self, buffer: &Buffer) {
        let block = &buffer.block(self);
        unsafe { block.unmap(&self.inner.logical) }
    }

    #[tracing::instrument(skip(data))]
    pub fn write_memory<T>(&self, buffer: &Buffer, offset: u64, data: &[T]) {
        let memory = self.map_memory(buffer, offset, size_of_val(data));

        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const _,
                memory.as_mut_ptr(),
                size_of_val(data),
            );
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateRenderPassError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error(
        "Subpass {subpass} attachment index {attachment} for color attachment {index} is out of bounds"
    )]
    ColorAttachmentReferenceOutOfBound {
        subpass: usize,
        index: usize,
        attachment: usize,
    },

    #[error(
        "Subpass {subpass} attachment index {attachment} for depth attachment is out of bounds"
    )]
    DepthAttachmentReferenceOutOfBound { subpass: usize, attachment: usize },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[cfg(target_arch = "wasm32")]
        source: Box<dyn Error + 'static>,

        #[cfg(not(target_arch = "wasm32"))]
        source: Box<dyn Error + Send + Sync + 'static>,
    },
}

#[allow(dead_code)]
fn check() {
    assert_object::<Device>();
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

pub(crate) fn create_render_pass_error_from_erupt(
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

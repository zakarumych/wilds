use crate::{
    arith_ne, assert_error, assert_object,
    buffer::{Buffer, BufferInfo, BufferUsage},
    descriptor::{
        CopyDescriptorSet, DescriptorSet, DescriptorSetInfo,
        DescriptorSetLayout, DescriptorSetLayoutInfo, WriteDescriptorSet,
    },
    fence::Fence,
    format::Format,
    image::{
        Image, ImageExtent, ImageInfo, ImageSubresource, ImageUsage, ImageView,
        ImageViewInfo, Samples,
    },
    memory::MemoryUsageFlags,
    pipeline::{
        AccelerationStructure, AccelerationStructureBuildGeometryInfo,
        AccelerationStructureInfo, GraphicsPipeline, GraphicsPipelineInfo,
        PipelineLayout, PipelineLayoutInfo, RayTracingPipeline,
        RayTracingPipelineInfo, ShaderBindingTable, ShaderBindingTableInfo,
    },
    render_pass::{Framebuffer, FramebufferInfo, RenderPass, RenderPassInfo},
    sampler::{Sampler, SamplerInfo},
    semaphore::Semaphore,
    shader::{CreateShaderModuleError, ShaderModule, ShaderModuleInfo},
    surface::{PresentMode, Surface, SurfaceError, Swapchain, SwapchainImage},
    DeviceAddress, Extent2d, OutOfMemory,
};
use bytemuck::{cast_slice, Pod};
use smallvec::SmallVec;
use std::{
    borrow::Borrow,
    collections::hash_map::{Entry, HashMap},
    convert::TryInto as _,
    error::Error,
    fmt::{self, Debug},
    mem::{size_of_val, MaybeUninit},
    ops::Range,
    sync::Arc,
};

#[derive(Debug, thiserror::Error)]
pub enum CreateDeviceError<E: Error + 'static> {
    #[error("{source}")]
    OutOfMemoryError {
        #[from]
        source: OutOfMemory,
    },

    #[error("Non-existed families are requested")]
    BadFamiliesRequested,

    #[error("{source}")]
    CannotFindRequeredQueues { source: E },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
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

/// Opaque value that represents graphics API device.
/// It is used to manage (create, destroy, check state) most of the device
/// resources.
pub struct Device {
    inner: Arc<dyn DeviceTrait>,
}

impl Debug for Device {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Device")
            .field("inner", &&*self.inner)
            .finish()
    }
}

impl Device {
    pub fn new(inner: Arc<impl DeviceTrait>) -> Self {
        Device { inner }
    }
}

impl Device {
    /// Creates buffer with uninitialized content.
    #[tracing::instrument]
    pub fn create_buffer(
        &self,
        info: BufferInfo,
    ) -> Result<Buffer, OutOfMemory> {
        assert!(info.is_valid());
        self.inner.clone().create_buffer(info)
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
        assert!(info.is_valid());
        if arith_ne(info.size, size_of_val(data)) {
            panic!(
                "Buffer size {} does not match data size {}",
                info.size,
                data.len()
            );
        }

        self.inner
            .clone()
            .create_buffer_static(info, cast_slice(data))
    }

    /// Creates a fence.
    /// Fences are create in unsignaled state.
    #[tracing::instrument]
    pub fn create_fence(&self) -> Result<Fence, OutOfMemory> {
        self.inner.clone().create_fence()
    }

    /// Creates framebuffer for specified render pass from views.
    #[tracing::instrument]
    pub fn create_framebuffer(
        &self,
        info: FramebufferInfo,
    ) -> Result<Framebuffer, OutOfMemory> {
        self.inner.clone().create_framebuffer(info)
    }

    /// Creates graphics pipeline.
    #[tracing::instrument]
    pub fn create_graphics_pipeline(
        &self,
        info: GraphicsPipelineInfo,
    ) -> Result<GraphicsPipeline, OutOfMemory> {
        self.inner.clone().create_graphics_pipeline(info)
    }

    /// Creates image with uninitialized content.
    #[tracing::instrument]
    pub fn create_image(
        &self,
        info: ImageInfo,
    ) -> Result<Image, CreateImageError> {
        self.inner.clone().create_image(info)
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
        // assert!(info.is_valid());
        self.inner
            .clone()
            .create_image_static(info, cast_slice(data))
    }

    /// Creates view to an image.
    #[tracing::instrument]
    pub fn create_image_view(
        &self,
        info: ImageViewInfo,
    ) -> Result<ImageView, OutOfMemory> {
        self.inner.clone().create_image_view(info)
    }

    /// Creates pipeline layout.
    #[tracing::instrument]
    pub fn create_pipeline_layout(
        &self,
        info: PipelineLayoutInfo,
    ) -> Result<PipelineLayout, OutOfMemory> {
        self.inner.clone().create_pipeline_layout(info)
    }

    /// Creates render pass.
    #[tracing::instrument]
    pub fn create_render_pass(
        &self,
        info: RenderPassInfo,
    ) -> Result<RenderPass, CreateRenderPassError> {
        self.inner.clone().create_render_pass(info)
    }

    /// Creates semaphore. Semaphores are created in unsignaled state.
    #[tracing::instrument]
    pub fn create_semaphore(&self) -> Result<Semaphore, OutOfMemory> {
        self.inner.clone().create_semaphore()
    }

    #[tracing::instrument]
    pub fn create_shader_module(
        &self,
        info: ShaderModuleInfo,
    ) -> Result<ShaderModule, CreateShaderModuleError> {
        self.inner.clone().create_shader_module(info)
    }

    /// Creates swapchain for specified surface.
    /// Only one swapchain may be associated with one surface.
    #[tracing::instrument]
    pub fn create_swapchain(
        &self,
        surface: &mut Surface,
    ) -> Result<Swapchain, SurfaceError> {
        self.inner.clone().create_swapchain(surface)
    }

    /// Resets fences.
    /// All specified fences must be in signalled state.
    /// Fences are moved into unsignalled state.
    #[tracing::instrument]
    pub fn reset_fences(&self, fences: &[&Fence]) {
        self.inner.reset_fences(fences)
    }

    #[tracing::instrument]
    pub fn is_fence_signalled(&self, fence: &Fence) -> bool {
        self.inner.is_fence_signalled(fence)
    }

    /// Wait for fences to become signaled.
    /// If `all` is `true` - waits for all specified fences to become signaled.
    /// Otherwise waits for at least on of specified fences to become signaled.
    /// May return immediately if all fences are already signaled (or at least
    /// one is signaled if `all == false`). Fences are signaled by `Queue`s.
    /// See `Queue::submit`.
    #[tracing::instrument]
    pub fn wait_fences(&self, fences: &[&Fence], all: bool) {
        self.inner.wait_fences(fences, all)
    }

    /// Wait for whole device to become idle. That is, wait for all pending
    /// operations to complete. This is equivalent to calling
    /// `Queue::wait_idle` for all queues. Typically used only before device
    /// destruction.
    #[tracing::instrument]
    pub fn wait_idle(&self) {
        self.inner.wait_idle();
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
        self.inner.clone().create_acceleration_structure(info)
    }

    /// Returns buffers device address.
    #[tracing::instrument]
    pub fn get_buffer_device_address(
        &self,
        buffer: &Buffer,
    ) -> Option<DeviceAddress> {
        self.inner.get_buffer_device_address(buffer)
    }

    #[tracing::instrument]
    pub fn get_acceleration_structure_device_address(
        &self,
        acceleration_structure: &AccelerationStructure,
    ) -> DeviceAddress {
        self.inner
            .get_acceleration_structure_device_address(acceleration_structure)
    }

    #[tracing::instrument]
    pub fn allocate_acceleration_structure_build_scratch(
        &self,
        acceleration_structure: &AccelerationStructure,
        update: bool,
    ) -> Result<Buffer, OutOfMemory> {
        self.inner
            .clone()
            .allocate_acceleration_structure_build_scratch(
                acceleration_structure,
                update,
            )
    }

    #[tracing::instrument]
    pub fn create_ray_tracing_pipeline(
        &self,
        info: RayTracingPipelineInfo,
    ) -> Result<RayTracingPipeline, OutOfMemory> {
        self.inner.clone().create_ray_tracing_pipeline(info)
    }

    #[tracing::instrument]
    pub fn create_descriptor_set_layout(
        &self,
        info: DescriptorSetLayoutInfo,
    ) -> Result<DescriptorSetLayout, OutOfMemory> {
        self.inner.clone().create_descriptor_set_layout(info)
    }

    #[tracing::instrument]
    pub fn create_descriptor_set(
        &self,
        info: DescriptorSetInfo,
    ) -> Result<DescriptorSet, OutOfMemory> {
        self.inner.clone().create_descriptor_set(info)
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

        self.inner.update_descriptor_sets(&writes, &copies)
    }

    #[tracing::instrument]
    pub fn create_sampler(
        &self,
        info: SamplerInfo,
    ) -> Result<Sampler, OutOfMemory> {
        self.inner.clone().create_sampler(info)
    }

    #[tracing::instrument]
    pub fn create_ray_tracing_shader_binding_table(
        &self,
        pipeline: &RayTracingPipeline,
        info: ShaderBindingTableInfo,
    ) -> Result<ShaderBindingTable, OutOfMemory> {
        self.inner
            .clone()
            .create_ray_tracing_shader_binding_table(pipeline, info)
    }

    #[tracing::instrument]
    pub fn map_memory(
        &self,
        buffer: &Buffer,
        offset: u64,
        size: usize,
    ) -> &mut [MaybeUninit<u8>] {
        self.inner.map_memory(buffer, offset, size)
    }

    #[tracing::instrument(skip(data))]
    pub fn write_memory<T>(&self, buffer: &Buffer, offset: u64, data: &[T])
    where
        T: Pod,
    {
        let memory = self.inner.map_memory(buffer, offset, size_of_val(data));

        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const _,
                memory.as_mut_ptr(),
                size_of_val(data),
            );
        }
    }
}

#[derive(Debug)]
pub enum CreateDeviceImplError {
    OutOfMemory {
        source: OutOfMemory,
    },

    BadFamiliesRequested,

    /// Implementation specific error.
    Other {
        #[cfg(target_arch = "wasm32")]
        source: Box<dyn Error + 'static>,

        #[cfg(not(target_arch = "wasm32"))]
        source: Box<dyn Error + Send + Sync + 'static>,
    },
}

impl<E> From<CreateDeviceImplError> for CreateDeviceError<E>
where
    E: Error + 'static,
{
    fn from(err: CreateDeviceImplError) -> Self {
        match err {
            CreateDeviceImplError::OutOfMemory { source } => {
                Self::OutOfMemoryError { source }
            }
            CreateDeviceImplError::BadFamiliesRequested => {
                Self::BadFamiliesRequested
            }
            CreateDeviceImplError::Other { source } => Self::Other { source },
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

pub trait DeviceTrait: Debug + Send + Sync + 'static {
    fn create_buffer(
        self: Arc<Self>,
        info: BufferInfo,
    ) -> Result<Buffer, OutOfMemory>;

    fn create_buffer_static(
        self: Arc<Self>,
        info: BufferInfo,
        data: &[u8],
    ) -> Result<Buffer, OutOfMemory>;

    fn create_fence(self: Arc<Self>) -> Result<Fence, OutOfMemory>;

    fn create_framebuffer(
        self: Arc<Self>,
        info: FramebufferInfo,
    ) -> Result<Framebuffer, OutOfMemory>;

    fn create_graphics_pipeline(
        self: Arc<Self>,
        info: GraphicsPipelineInfo,
    ) -> Result<GraphicsPipeline, OutOfMemory>;

    fn create_image(
        self: Arc<Self>,
        info: ImageInfo,
    ) -> Result<Image, CreateImageError>;

    fn create_image_static(
        self: Arc<Self>,
        info: ImageInfo,
        data: &[u8],
    ) -> Result<Image, CreateImageError>;

    fn create_image_view(
        self: Arc<Self>,
        info: ImageViewInfo,
    ) -> Result<ImageView, OutOfMemory>;

    fn create_pipeline_layout(
        self: Arc<Self>,
        info: PipelineLayoutInfo,
    ) -> Result<PipelineLayout, OutOfMemory>;

    fn create_render_pass(
        self: Arc<Self>,
        render_pass_info: RenderPassInfo,
    ) -> Result<RenderPass, CreateRenderPassError>;

    fn create_semaphore(self: Arc<Self>) -> Result<Semaphore, OutOfMemory>;

    fn create_shader_module(
        self: Arc<Self>,
        info: ShaderModuleInfo,
    ) -> Result<ShaderModule, CreateShaderModuleError>;

    fn create_swapchain(
        self: Arc<Self>,
        surface: &mut Surface,
    ) -> Result<Swapchain, SurfaceError>;

    fn reset_fences(&self, fences: &[&Fence]);

    fn is_fence_signalled(&self, fence: &Fence) -> bool;

    fn wait_fences(&self, fences: &[&Fence], all: bool);

    fn wait_idle(&self);

    fn create_acceleration_structure(
        self: Arc<Self>,
        info: AccelerationStructureInfo,
    ) -> Result<AccelerationStructure, OutOfMemory>;

    fn get_buffer_device_address(
        &self,
        buffer: &Buffer,
    ) -> Option<DeviceAddress>;

    fn get_acceleration_structure_device_address(
        &self,
        acceleration_structure: &AccelerationStructure,
    ) -> DeviceAddress;

    fn allocate_acceleration_structure_build_scratch(
        self: Arc<Self>,
        acceleration_structure: &AccelerationStructure,
        update: bool,
    ) -> Result<Buffer, OutOfMemory>;

    fn create_ray_tracing_pipeline(
        self: Arc<Self>,
        info: RayTracingPipelineInfo,
    ) -> Result<RayTracingPipeline, OutOfMemory>;

    fn create_descriptor_set_layout(
        self: Arc<Self>,
        info: DescriptorSetLayoutInfo,
    ) -> Result<DescriptorSetLayout, OutOfMemory>;

    fn create_descriptor_set(
        self: Arc<Self>,
        info: DescriptorSetInfo,
    ) -> Result<DescriptorSet, OutOfMemory>;

    fn update_descriptor_sets(
        &self,
        writes: &[WriteDescriptorSet<'_>],
        copies: &[CopyDescriptorSet<'_>],
    );

    fn create_sampler(
        self: Arc<Self>,
        info: SamplerInfo,
    ) -> Result<Sampler, OutOfMemory>;

    fn create_ray_tracing_shader_binding_table(
        self: Arc<Self>,
        pipeline: &RayTracingPipeline,
        info: ShaderBindingTableInfo,
    ) -> Result<ShaderBindingTable, OutOfMemory>;

    fn map_memory(
        &self,
        buffer: &Buffer,
        offset: u64,
        size: usize,
    ) -> &mut [MaybeUninit<u8>];

    fn unmap_memory(&self, buffer: &Buffer);
}

#[allow(dead_code)]
fn check() {
    assert_object::<Device>();
}

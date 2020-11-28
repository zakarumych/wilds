use {
    super::{descriptor::DescriptorSizes, device::WeakDevice},
    crate::{
        accel::AccelerationStructureInfo,
        buffer::BufferInfo,
        descriptor::{DescriptorSetInfo, DescriptorSetLayoutInfo},
        framebuffer::FramebufferInfo,
        image::ImageInfo,
        memory::MemoryUsage,
        pipeline::{
            ComputePipelineInfo, GraphicsPipelineInfo, PipelineLayoutInfo,
            RayTracingPipelineInfo,
        },
        render_pass::RenderPassInfo,
        sampler::SamplerInfo,
        shader::ShaderModuleInfo,
        view::ImageViewInfo,
        DeviceAddress,
    },
    erupt::{extensions::khr_acceleration_structure as vkacc, vk1_0},
    gpu_alloc::MemoryBlock,
    std::{
        cell::UnsafeCell,
        fmt::{self, Debug},
        hash::{Hash, Hasher},
        ops::Deref,
        sync::Arc,
    },
};

struct BufferInner {
    info: BufferInfo,
    owner: WeakDevice,
    handle: vk1_0::Buffer,
    address: Option<DeviceAddress>,
    index: usize,
    memory_handle: vk1_0::DeviceMemory,
    memory_offset: u64,
    memory_size: u64,
    memory_block: UnsafeCell<MemoryBlock<vk1_0::DeviceMemory>>,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Buffer {
    inner: Arc<BufferInner>,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl PartialEq for Buffer {
    fn eq(&self, rhs: &Self) -> bool {
        self.inner.handle == rhs.inner.handle
    }
}

impl Eq for Buffer {}

impl Hash for Buffer {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.inner.handle.hash(hasher)
    }
}

impl Debug for Buffer {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Memory {
            handle: vk1_0::DeviceMemory,
            offset: u64,
            size: u64,
        }

        fmt.debug_struct("Buffer")
            .field("info", &self.inner.info)
            .field("owner", &self.inner.owner)
            .field("handle", &self.inner.handle)
            .field("address", &self.inner.address)
            .field("index", &self.inner.index)
            .field(
                "memory",
                &Memory {
                    handle: self.inner.memory_handle,
                    offset: self.inner.memory_offset,
                    size: self.inner.memory_size,
                },
            )
            .finish()
    }
}

impl Buffer {
    pub fn info(&self) -> &BufferInfo {
        &self.inner.info
    }

    pub fn address(&self) -> Option<DeviceAddress> {
        self.inner.address
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.inner.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.inner.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Buffer {
        self.inner.handle
    }
}

pub struct MappableBuffer {
    buffer: Buffer,
    memory_usage: MemoryUsage,
}

impl From<MappableBuffer> for Buffer {
    fn from(buffer: MappableBuffer) -> Self {
        buffer.buffer
    }
}

impl PartialEq for MappableBuffer {
    fn eq(&self, rhs: &Self) -> bool {
        std::ptr::eq(self, rhs)
    }
}

impl Eq for MappableBuffer {}

impl Hash for MappableBuffer {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.buffer.inner.handle.hash(hasher)
    }
}

impl Debug for MappableBuffer {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Memory {
            handle: vk1_0::DeviceMemory,
            offset: u64,
            size: u64,
            usage: MemoryUsage,
        }

        fmt.debug_struct("Buffer")
            .field("info", &self.inner.info)
            .field("owner", &self.inner.owner)
            .field("handle", &self.inner.handle)
            .field("address", &self.inner.address)
            .field("index", &self.inner.index)
            .field(
                "memory",
                &Memory {
                    handle: self.inner.memory_handle,
                    offset: self.inner.memory_offset,
                    size: self.inner.memory_size,
                    usage: self.memory_usage,
                },
            )
            .finish()
    }
}

impl Deref for MappableBuffer {
    type Target = Buffer;

    fn deref(&self) -> &Buffer {
        &self.buffer
    }
}

impl MappableBuffer {
    pub fn share(&self) -> Buffer {
        Buffer {
            inner: self.inner.clone(),
        }
    }

    pub(super) fn new(
        info: BufferInfo,
        owner: WeakDevice,
        handle: vk1_0::Buffer,
        address: Option<DeviceAddress>,
        index: usize,
        memory_block: MemoryBlock<vk1_0::DeviceMemory>,
        memory_usage: MemoryUsage,
    ) -> Self {
        MappableBuffer {
            buffer: Buffer {
                inner: Arc::new(BufferInner {
                    info,
                    owner,
                    handle,
                    address,
                    memory_handle: *memory_block.memory(),
                    memory_offset: memory_block.offset(),
                    memory_size: memory_block.size(),
                    memory_block: UnsafeCell::new(memory_block),
                    index,
                }),
            },
            memory_usage,
        }
    }

    /// # Safety
    ///
    /// MemoryBlock must not be replaced
    pub(super) unsafe fn memory_block(
        &mut self,
    ) -> &mut MemoryBlock<vk1_0::DeviceMemory> {
        // exclusive access
        &mut *self.inner.memory_block.get()
    }
}

#[derive(Debug)]
struct ImageInner {
    info: ImageInfo,
    owner: WeakDevice,
    handle: vk1_0::Image,
    memory_block: Option<MemoryBlock<vk1_0::DeviceMemory>>,
    index: Option<usize>,
}

#[derive(Clone)]
pub struct Image {
    inner: Arc<ImageInner>,
}

impl PartialEq for Image {
    fn eq(&self, rhs: &Self) -> bool {
        self.inner.handle == rhs.inner.handle
    }
}

impl Eq for Image {}

impl Hash for Image {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.inner.handle.hash(hasher)
    }
}

impl Debug for Image {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(fmt)
    }
}

impl Image {
    pub fn info(&self) -> &ImageInfo {
        &self.inner.info
    }

    pub(super) fn new(
        info: ImageInfo,
        owner: WeakDevice,
        handle: vk1_0::Image,
        memory_block: Option<MemoryBlock<vk1_0::DeviceMemory>>,
        index: Option<usize>,
    ) -> Self {
        Image {
            inner: Arc::new(ImageInner {
                info,
                owner,
                handle,
                memory_block,
                index,
            }),
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.inner.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.inner.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Image {
        self.inner.handle
    }
}

#[derive(Clone, Debug)]
pub struct ImageView {
    info: ImageViewInfo,
    owner: WeakDevice,
    handle: vk1_0::ImageView,
    index: usize,
}

impl PartialEq for ImageView {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for ImageView {}

impl Hash for ImageView {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl ImageView {
    pub fn info(&self) -> &ImageViewInfo {
        &self.info
    }

    pub(super) fn new(
        info: ImageViewInfo,
        owner: WeakDevice,
        handle: vk1_0::ImageView,
        index: usize,
    ) -> Self {
        ImageView {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::ImageView {
        self.handle
    }
}

#[derive(Clone, Debug)]
pub struct Fence {
    owner: WeakDevice,
    handle: vk1_0::Fence,
    index: usize,
}

impl PartialEq for Fence {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for Fence {}

impl Hash for Fence {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl Fence {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk1_0::Fence,
        index: usize,
    ) -> Self {
        Fence {
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Fence {
        self.handle
    }
}

#[derive(Clone, Debug)]
pub struct Semaphore {
    owner: WeakDevice,
    handle: vk1_0::Semaphore,
    index: usize,
}

impl PartialEq for Semaphore {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for Semaphore {}

impl Hash for Semaphore {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl Semaphore {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk1_0::Semaphore,
        index: usize,
    ) -> Self {
        Semaphore {
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Semaphore {
        self.handle
    }
}

/// Render pass represents collection of attachments,
/// subpasses, and dependencies between subpasses,
/// and describes how they are used over the course of the subpasses.
///
/// This value is handle to a render pass resource.
#[derive(Clone, Debug)]
pub struct RenderPass {
    info: RenderPassInfo,
    owner: WeakDevice,
    handle: vk1_0::RenderPass,
    index: usize,
}

impl PartialEq for RenderPass {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for RenderPass {}

impl Hash for RenderPass {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl RenderPass {
    pub fn info(&self) -> &RenderPassInfo {
        &self.info
    }

    pub(super) fn new(
        info: RenderPassInfo,
        owner: WeakDevice,
        handle: vk1_0::RenderPass,
        index: usize,
    ) -> Self {
        RenderPass {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::RenderPass {
        self.handle
    }
}

#[derive(Clone, Debug)]
pub struct Sampler {
    info: SamplerInfo,
    owner: WeakDevice,
    handle: vk1_0::Sampler,
    index: usize,
}

impl PartialEq for Sampler {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for Sampler {}

impl Hash for Sampler {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl Sampler {
    pub fn info(&self) -> &SamplerInfo {
        &self.info
    }

    pub(super) fn new(
        info: SamplerInfo,
        owner: WeakDevice,
        handle: vk1_0::Sampler,
        index: usize,
    ) -> Self {
        Sampler {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Sampler {
        self.handle
    }
}

/// Framebuffer is a collection of attachments for render pass.
/// Images format and sample count should match attachment definitions.
/// All image views must be 2D with 1 mip level and 1 array level.
#[derive(Clone, Debug)]
pub struct Framebuffer {
    info: FramebufferInfo,
    owner: WeakDevice,
    handle: vk1_0::Framebuffer,
    index: usize,
}

impl PartialEq for Framebuffer {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for Framebuffer {}

impl Hash for Framebuffer {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl Framebuffer {
    pub fn info(&self) -> &FramebufferInfo {
        &self.info
    }

    pub(super) fn new(
        info: FramebufferInfo,
        owner: WeakDevice,
        handle: vk1_0::Framebuffer,
        index: usize,
    ) -> Self {
        Framebuffer {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Framebuffer {
        self.handle
    }
}

/// Resource that describes layout for descriptor sets.
#[derive(Clone, Debug)]
pub struct ShaderModule {
    info: ShaderModuleInfo,
    owner: WeakDevice,
    handle: vk1_0::ShaderModule,
    index: usize,
}

impl PartialEq for ShaderModule {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for ShaderModule {}

impl Hash for ShaderModule {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl ShaderModule {
    pub fn info(&self) -> &ShaderModuleInfo {
        &self.info
    }

    pub(super) fn new(
        info: ShaderModuleInfo,
        owner: WeakDevice,
        handle: vk1_0::ShaderModule,
        index: usize,
    ) -> Self {
        ShaderModule {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::ShaderModule {
        self.handle
    }
}

/// Resource that describes layout for descriptor sets.
#[derive(Clone, Debug)]
pub struct DescriptorSetLayout {
    info: DescriptorSetLayoutInfo,
    owner: WeakDevice,
    handle: vk1_0::DescriptorSetLayout,
    sizes: DescriptorSizes,
    index: usize,
}

impl PartialEq for DescriptorSetLayout {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for DescriptorSetLayout {}

impl Hash for DescriptorSetLayout {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl DescriptorSetLayout {
    pub fn info(&self) -> &DescriptorSetLayoutInfo {
        &self.info
    }

    pub(super) fn new(
        info: DescriptorSetLayoutInfo,
        owner: WeakDevice,
        handle: vk1_0::DescriptorSetLayout,
        sizes: DescriptorSizes,
        index: usize,
    ) -> Self {
        DescriptorSetLayout {
            info,
            owner,
            handle,
            sizes,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::DescriptorSetLayout {
        self.handle
    }

    pub(super) fn sizes(&self) -> &DescriptorSizes {
        &self.sizes
    }
}

/// Set of descriptors with specific layout.
#[derive(Clone, Debug)]
pub struct DescriptorSet {
    info: DescriptorSetInfo,
    owner: WeakDevice,
    handle: vk1_0::DescriptorSet,
    pool: vk1_0::DescriptorPool,
    pool_index: usize,
}

impl PartialEq for DescriptorSet {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for DescriptorSet {}

impl Hash for DescriptorSet {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl DescriptorSet {
    pub fn info(&self) -> &DescriptorSetInfo {
        &self.info
    }

    pub(super) fn new(
        info: DescriptorSetInfo,
        owner: WeakDevice,
        handle: vk1_0::DescriptorSet,
        pool: vk1_0::DescriptorPool,
        pool_index: usize,
    ) -> Self {
        DescriptorSet {
            info,
            owner,
            handle,
            pool,
            pool_index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::DescriptorSet {
        self.handle
    }
}

/// Resource that describes layout of a pipeline.
#[derive(Clone, Debug)]
pub struct PipelineLayout {
    info: PipelineLayoutInfo,
    owner: WeakDevice,
    handle: vk1_0::PipelineLayout,
    index: usize,
}

impl PartialEq for PipelineLayout {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for PipelineLayout {}

impl Hash for PipelineLayout {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl PipelineLayout {
    pub fn info(&self) -> &PipelineLayoutInfo {
        &self.info
    }

    pub(super) fn new(
        info: PipelineLayoutInfo,
        owner: WeakDevice,
        handle: vk1_0::PipelineLayout,
        index: usize,
    ) -> Self {
        PipelineLayout {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::PipelineLayout {
        self.handle
    }
}

/// Resource that describes whole compute pipeline state.
#[derive(Clone, Debug)]
pub struct ComputePipeline {
    info: ComputePipelineInfo,
    owner: WeakDevice,
    handle: vk1_0::Pipeline,
    index: usize,
}

impl PartialEq for ComputePipeline {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for ComputePipeline {}

impl Hash for ComputePipeline {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl ComputePipeline {
    pub fn info(&self) -> &ComputePipelineInfo {
        &self.info
    }

    pub(super) fn new(
        info: ComputePipelineInfo,
        owner: WeakDevice,
        handle: vk1_0::Pipeline,
        index: usize,
    ) -> Self {
        ComputePipeline {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Pipeline {
        self.handle
    }
}

/// Resource that describes whole graphics pipeline state.
#[derive(Clone, Debug)]
pub struct GraphicsPipeline {
    info: GraphicsPipelineInfo,
    owner: WeakDevice,
    handle: vk1_0::Pipeline,
    index: usize,
}

impl PartialEq for GraphicsPipeline {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for GraphicsPipeline {}

impl Hash for GraphicsPipeline {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl GraphicsPipeline {
    pub fn info(&self) -> &GraphicsPipelineInfo {
        &self.info
    }

    pub(super) fn new(
        info: GraphicsPipelineInfo,
        owner: WeakDevice,
        handle: vk1_0::Pipeline,
        index: usize,
    ) -> Self {
        GraphicsPipeline {
            info,
            owner,
            handle,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Pipeline {
        self.handle
    }
}

/// Bottom-level acceleration structure.
#[derive(Clone, Debug)]
pub struct AccelerationStructure {
    info: AccelerationStructureInfo,
    owner: WeakDevice,
    handle: vkacc::AccelerationStructureKHR,
    address: DeviceAddress,
    index: usize,
}

impl PartialEq for AccelerationStructure {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for AccelerationStructure {}

impl Hash for AccelerationStructure {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl AccelerationStructure {
    pub fn info(&self) -> &AccelerationStructureInfo {
        &self.info
    }

    pub fn address(&self) -> DeviceAddress {
        self.address
    }

    pub(super) fn new(
        info: AccelerationStructureInfo,
        owner: WeakDevice,
        handle: vkacc::AccelerationStructureKHR,
        address: DeviceAddress,
        index: usize,
    ) -> Self {
        AccelerationStructure {
            info,
            owner,
            handle,
            address,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vkacc::AccelerationStructureKHR {
        self.handle
    }
}

/// Resource that describes whole ray-tracing pipeline state.
#[derive(Clone, Debug)]
pub struct RayTracingPipeline {
    info: RayTracingPipelineInfo,
    owner: WeakDevice,
    handle: vk1_0::Pipeline,
    group_handlers: Arc<[u8]>,
    index: usize,
}

impl PartialEq for RayTracingPipeline {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle == rhs.handle
    }
}

impl Eq for RayTracingPipeline {}

impl Hash for RayTracingPipeline {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.handle.hash(hasher)
    }
}

impl RayTracingPipeline {
    pub fn info(&self) -> &RayTracingPipelineInfo {
        &self.info
    }

    pub(super) fn new(
        info: RayTracingPipelineInfo,
        owner: WeakDevice,
        handle: vk1_0::Pipeline,
        group_handlers: Arc<[u8]>,
        index: usize,
    ) -> Self {
        RayTracingPipeline {
            info,
            owner,
            handle,
            group_handlers,
            index,
        }
    }

    pub(super) fn is_owned_by(
        &self,
        owner: &impl PartialEq<WeakDevice>,
    ) -> bool {
        *owner == self.owner
    }

    pub(super) fn owner(&self) -> &WeakDevice {
        &self.owner
    }

    pub(super) fn handle(&self) -> vk1_0::Pipeline {
        self.handle
    }

    pub(super) fn group_handlers(&self) -> &[u8] {
        &*self.group_handlers
    }
}

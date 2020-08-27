use {
    super::{descriptor::DescriptorSizes, device::WeakDevice},
    crate::{
        accel::AccelerationStructureInfo,
        buffer::BufferInfo,
        descriptor::{DescriptorSetInfo, DescriptorSetLayoutInfo},
        fence::FenceInfo,
        framebuffer::FramebufferInfo,
        image::ImageInfo,
        pipeline::{
            ComputePipelineInfo, GraphicsPipelineInfo, PipelineLayoutInfo,
            RayTracingPipelineInfo,
        },
        render_pass::RenderPassInfo,
        sampler::SamplerInfo,
        semaphore::SemaphoreInfo,
        shader::ShaderModuleInfo,
        view::ImageViewInfo,
        DeviceAddress,
    },
    erupt::{extensions::khr_ray_tracing as vkrt, vk1_0},
    tvma::Block,
};

define_handle! {
    pub struct Buffer : BufferInner {
        pub info: BufferInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Buffer,
        address: Option<DeviceAddress>,
        block: Block,
        index: usize,
    }
}

define_handle! {
    pub struct Image : ImageInner {
        pub info: ImageInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Image,
        block: Option<Block>,
        index: Option<usize>,
    }
}

define_handle! {
    pub struct Fence : FenceInner {
        pub info: FenceInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Fence,
        index: usize,
    }
}

define_handle! {
    /// Bottom-level acceleration structure.
    pub struct AccelerationStructure : AccelerationStructureInner {
        pub info: AccelerationStructureInfo,
        pub owner: WeakDevice,
        handle: vkrt::AccelerationStructureKHR,
        address: DeviceAddress,
        block: Block,
        index: usize,
    }
}

define_handle! {
    pub struct ImageView : ImageViewInner {
        pub info: ImageViewInfo,
        pub owner: WeakDevice,
        handle: vk1_0::ImageView,
        index: usize,
    }
}

define_handle! {
    pub struct Semaphore : SemaphoreInner {
        pub info: SemaphoreInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Semaphore,
        index: usize,
    }
}

define_handle! {
    /// Render pass represents collection of attachments,
    /// subpasses, and dependencies between subpasses,
    /// and describes how they are used over the course of the subpasses.
    ///
    /// This value is handle to a render pass resource.
    pub struct RenderPass : RenderPassInner {
        pub info: RenderPassInfo,
        pub owner: WeakDevice,
        handle: vk1_0::RenderPass,
        index: usize,
    }
}

define_handle! {
    pub struct Sampler : SamplerInner {
        pub info: SamplerInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Sampler,
        index: usize,
    }
}

define_handle! {
    /// Framebuffer is a collection of attachments for render pass.
    /// Images format and sample count should match attachment definitions.
    /// All image views must be 2D with 1 mip level and 1 array level.
    pub struct Framebuffer : FramebufferInner {
        pub info: FramebufferInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Framebuffer,
        index: usize,
    }
}

define_handle! {
    /// Resource that describes layout for descriptor sets.
    pub struct ShaderModule : ShaderModuleInner {
        pub info: ShaderModuleInfo,
        pub owner: WeakDevice,
        handle: vk1_0::ShaderModule,
        index: usize,
    }
}

define_handle! {
    /// Resource that describes layout for descriptor sets.
    pub struct DescriptorSetLayout : DescriptorSetLayoutInner {
        pub info: DescriptorSetLayoutInfo,
        pub owner: WeakDevice,
        handle: vk1_0::DescriptorSetLayout,
        sizes: DescriptorSizes,
        index: usize,
    }
}

define_handle! {
    /// Set of descriptors with specific layout.
    pub struct DescriptorSet : DescriptorSetInner {
        pub info: DescriptorSetInfo,
        pub owner: WeakDevice,
        handle: vk1_0::DescriptorSet,
        pool: vk1_0::DescriptorPool,
        pool_index: usize,
    }
}

define_handle! {
    /// Resource that describes layout of a pipeline.
    pub struct PipelineLayout : PipelineLayoutInner {
        pub info: PipelineLayoutInfo,
        pub owner: WeakDevice,
        handle: vk1_0::PipelineLayout,
        index: usize,
    }
}

define_handle! {
    /// Resource that describes whole compute pipeline state.
    pub struct ComputePipeline : ComputePipelineInner {
        pub info: ComputePipelineInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Pipeline,
        index: usize,
    }
}

define_handle! {
    /// Resource that describes whole ray-tracing pipeline state.
    pub struct RayTracingPipeline : RayTracingPipelineInner {
        pub info: RayTracingPipelineInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Pipeline,
        group_handlers: Box<[u8]>,
        index: usize,
    }
}

define_handle! {
    /// Resource that describes whole graphics pipeline state.
    pub struct GraphicsPipeline : GraphicsPipelineInner {
        pub info: GraphicsPipelineInfo,
        pub owner: WeakDevice,
        handle: vk1_0::Pipeline,
        index: usize,
    }
}

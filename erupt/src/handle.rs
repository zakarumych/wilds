use crate::{descriptor::DescriptorSizes, device::EruptDevice, EruptGraphics};
use erupt::{
    extensions::{
        khr_ray_tracing as vkrt, khr_surface::SurfaceKHR,
        khr_swapchain::SwapchainKHR,
    },
    vk1_0,
};
use illume::{
    AccelerationStructure, Buffer, DescriptorSet, DescriptorSetLayout,
    DeviceAddress, Fence, Framebuffer, GraphicsPipeline, Handle, Image,
    ImageView, PipelineLayout, RayTracingPipeline, RenderPass, ResourceTrait,
    Sampler, Semaphore, ShaderModule, Specific, Surface, SwapchainImage,
};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Weak,
};
use tvma::Block;

fn leaked<T: 'static>() {
    eprintln!("LEAKED: {}", std::any::type_name::<T>());
}

pub(super) unsafe trait EruptResource: ResourceTrait {
    type Owner: Sized;

    type Erupt: Specific<Self>;

    fn is_owner(&self, owner: &Self::Owner) -> bool;

    fn make(specific: Self::Erupt, info: Self::Info) -> Self {
        Self::from_handle(Handle::new(specific, info))
    }

    fn erupt_ref(&self, owner: &Self::Owner) -> &Self::Erupt {
        assert!(self.is_owner(owner), "Wrong owner");

        self.handle().specific_ref().expect("Wrong type")
    }

    unsafe fn erupt_ref_unchecked(&self) -> &Self::Erupt {
        self.handle().specific_ref_unchecked()
    }
}

#[derive(Debug)]
pub(super) struct EruptSurface {
    pub handle: SurfaceKHR,
    pub used: AtomicBool,
    pub owner: Weak<EruptGraphics>,
}

impl Drop for EruptSurface {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Surface> for EruptSurface {}

unsafe impl EruptResource for Surface {
    type Erupt = EruptSurface;
    type Owner = EruptGraphics;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptImage {
    pub handle: vk1_0::Image,
    pub block: Option<Block>,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptImage {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Image> for EruptImage {}

unsafe impl EruptResource for Image {
    type Erupt = EruptImage;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptImageView {
    pub handle: vk1_0::ImageView,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptImageView {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<ImageView> for EruptImageView {}

unsafe impl EruptResource for ImageView {
    type Erupt = EruptImageView;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptBuffer {
    pub handle: vk1_0::Buffer,
    pub address: Option<DeviceAddress>,
    pub block: Block,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptBuffer {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Buffer> for EruptBuffer {}

unsafe impl EruptResource for Buffer {
    type Erupt = EruptBuffer;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptSemaphore {
    pub handle: vk1_0::Semaphore,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptSemaphore {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Semaphore> for EruptSemaphore {}

unsafe impl EruptResource for Semaphore {
    type Erupt = EruptSemaphore;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptFence {
    pub handle: vk1_0::Fence,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptFence {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Fence> for EruptFence {}

unsafe impl EruptResource for Fence {
    type Erupt = EruptFence;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptRenderPass {
    pub handle: vk1_0::RenderPass,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptRenderPass {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<RenderPass> for EruptRenderPass {}

unsafe impl EruptResource for RenderPass {
    type Erupt = EruptRenderPass;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptFramebuffer {
    pub handle: vk1_0::Framebuffer,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptFramebuffer {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Framebuffer> for EruptFramebuffer {}

unsafe impl EruptResource for Framebuffer {
    type Erupt = EruptFramebuffer;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptShaderModule {
    pub handle: vk1_0::ShaderModule,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptShaderModule {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<ShaderModule> for EruptShaderModule {}

unsafe impl EruptResource for ShaderModule {
    type Erupt = EruptShaderModule;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptDescriptorSetLayout {
    pub handle: vk1_0::DescriptorSetLayout,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
    pub sizes: DescriptorSizes,
}

impl Drop for EruptDescriptorSetLayout {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<DescriptorSetLayout> for EruptDescriptorSetLayout {}

unsafe impl EruptResource for DescriptorSetLayout {
    type Erupt = EruptDescriptorSetLayout;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptDescriptorSet {
    pub handle: vk1_0::DescriptorSet,
    pub pool: vk1_0::DescriptorPool,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
    pub pool_index: usize,
}

impl Drop for EruptDescriptorSet {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<DescriptorSet> for EruptDescriptorSet {}

unsafe impl EruptResource for DescriptorSet {
    type Erupt = EruptDescriptorSet;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptPipelineLayout {
    pub handle: vk1_0::PipelineLayout,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptPipelineLayout {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<PipelineLayout> for EruptPipelineLayout {}

unsafe impl EruptResource for PipelineLayout {
    type Erupt = EruptPipelineLayout;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptGraphicsPipeline {
    pub handle: vk1_0::Pipeline,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptGraphicsPipeline {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<GraphicsPipeline> for EruptGraphicsPipeline {}

unsafe impl EruptResource for GraphicsPipeline {
    type Erupt = EruptGraphicsPipeline;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptSwapchainImage {
    pub swapchain: SwapchainKHR,
    pub index: u32,
    pub supported_families: Arc<Vec<bool>>,
    pub counter: Weak<AtomicUsize>,
    pub owner: Weak<EruptDevice>,
}

impl Drop for EruptSwapchainImage {
    fn drop(&mut self) {
        if let Some(counter) = self.counter.upgrade() {
            counter.fetch_sub(1, Ordering::Release);
        }
    }
}

impl Specific<SwapchainImage> for EruptSwapchainImage {}

unsafe impl EruptResource for SwapchainImage {
    type Erupt = EruptSwapchainImage;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptAccelerationStructure {
    pub handle: vkrt::AccelerationStructureKHR,
    pub address: DeviceAddress,
    pub block: Block,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptAccelerationStructure {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<AccelerationStructure> for EruptAccelerationStructure {}

unsafe impl EruptResource for AccelerationStructure {
    type Erupt = EruptAccelerationStructure;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptRayTracingPipeline {
    pub handle: vk1_0::Pipeline,
    pub owner: Weak<EruptDevice>,
    pub index: usize,

    pub group_handlers: Vec<u8>,
}

impl Drop for EruptRayTracingPipeline {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<RayTracingPipeline> for EruptRayTracingPipeline {}

unsafe impl EruptResource for RayTracingPipeline {
    type Erupt = EruptRayTracingPipeline;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

#[derive(Debug)]
pub(super) struct EruptSampler {
    pub handle: vk1_0::Sampler,
    pub owner: Weak<EruptDevice>,
    pub index: usize,
}

impl Drop for EruptSampler {
    fn drop(&mut self) {
        leaked::<Self>()
    }
}

impl Specific<Sampler> for EruptSampler {}

unsafe impl EruptResource for Sampler {
    type Erupt = EruptSampler;
    type Owner = EruptDevice;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::Erupt>()
            .and_then(|erupt| erupt.owner.upgrade())
            .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
    }
}

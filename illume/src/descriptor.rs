use crate::{
    buffer::Buffer,
    image::{Image, ImageView, Layout},
    pipeline::AccelerationStructure,
    resource::{Handle, ResourceTrait},
    sampler::Sampler,
    shader::ShaderStageFlags,
};

bitflags::bitflags! {
    /// Bits which can be set in each element of VkDescriptorSetLayoutBindingFlagsCreateInfo::pBindingFlags to specify options for the corresponding descriptor set layout binding are:
    /// Note that Vulkan 1.2 is required for any of the flags.
    // That is, the only valid value prior Vulkan 1.2 is `DescriptorBindingFlags::empty()`.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct DescriptorBindingFlags: u32 {
        const UPDATE_AFTER_BIND = 0x00000001;
        const UPDATE_UNUSED_WHILE_PENDING = 0x00000002;
        const PARTIALLY_BOUND = 0x00000004;
        const VARIABLE_DESCRIPTOR_COUNT = 0x00000008;
    }
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct DescriptorSetLayoutFlags: u32 {
        const PUSH_DESCRIPTOR = 0x00000001;
        const UPDATE_AFTER_BIND_POOL = 0x00000002;
    }
}

define_handle! {
    /// Resource that describes layout for descriptor sets.
    pub struct DescriptorSetLayout(DescriptorSetLayoutInfo);
}

/// Defines layout for descriptor sets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct DescriptorSetLayoutInfo {
    pub bindings: Vec<DescriptorSetLayoutBinding>,
    pub flags: DescriptorSetLayoutFlags,
}

/// Defines layout for one binding in descriptor set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct DescriptorSetLayoutBinding {
    /// Binding index.
    pub binding: u32,

    /// Type of descriptor in the binding.
    pub ty: DescriptorType,

    /// Number of dfescriptors in the binding.
    pub count: u32,

    /// Shader stages where this binding is accessible.
    pub stages: ShaderStageFlags,

    /// Flags to specify options for the descriptor set layout binding.
    pub flags: DescriptorBindingFlags,
}

/// Types of descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum DescriptorType {
    Sampler,
    CombinedImageSampler,
    SampledImage,
    StorageImage,
    UniformTexelBuffer,
    StorageTexelBuffer,
    UniformBuffer,
    StorageBuffer,
    UniformBufferDynamic,
    StorageBufferDynamic,
    InputAttachment,
    AccelerationStructure,
}

define_handle! {
    /// Set of descriptors with specific layout.
    pub struct DescriptorSet(DescriptorSetInfo);
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DescriptorSetInfo {
    pub layout: DescriptorSetLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WriteDescriptorSet<'a> {
    pub set: &'a DescriptorSet,
    pub binding: u32,
    pub element: u32,
    pub descriptors: Descriptors<'a>,
}

impl WriteDescriptorSet<'_> {
    pub fn validate(&self) {
        match self.descriptors {
            Descriptors::UniformBuffer(buffers)
            | Descriptors::StorageBuffer(buffers)
            | Descriptors::UniformBufferDynamic(buffers)
            | Descriptors::StorageBufferDynamic(buffers) => {
                for &(ref buffer, offset, size) in buffers {
                    debug_assert_ne!(size, 0);
                    debug_assert!(offset.checked_add(size).is_some());
                    debug_assert!(buffer.info().size >= offset + size);
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Descriptors<'a> {
    Sampler(&'a [Sampler]),
    CombinedImageSampler(&'a [(ImageView, Layout, Sampler)]),
    SampledImage(&'a [(ImageView, Layout)]),
    StorageImage(&'a [(ImageView, Layout)]),
    // UniformTexelBuffer(&'a BufferView),
    // StorageTexelBuffer(&'a BufferView),
    UniformBuffer(&'a [(Buffer, u64, u64)]),
    StorageBuffer(&'a [(Buffer, u64, u64)]),
    UniformBufferDynamic(&'a [(Buffer, u64, u64)]),
    StorageBufferDynamic(&'a [(Buffer, u64, u64)]),
    InputAttachment(&'a [(ImageView, Layout)]),
    AccelerationStructure(&'a [AccelerationStructure]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CopyDescriptorSet<'a> {
    pub src: &'a DescriptorSet,
    pub src_binding: u32,
    pub src_element: u32,
    pub dst: &'a DescriptorSet,
    pub dst_binding: u32,
    pub dst_element: u32,
    pub count: u32,
}

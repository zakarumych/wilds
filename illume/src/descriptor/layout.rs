use crate::shader::ShaderStageFlags;
use erupt::vk1_0;
use std::{
    hash::{Hash, Hasher},
    ops::Deref,
};

const DESCRIPTOR_TYPES_COUNT: usize = 12;

fn descriptor_type_from_index(index: usize) -> vk1_0::DescriptorType {
    debug_assert!(index < DESCRIPTOR_TYPES_COUNT);

    match index {
        0 => {
            debug_assert_eq!(DescriptorType::Sampler as usize, index);

            vk1_0::DescriptorType::SAMPLER
        }
        1 => {
            debug_assert_eq!(
                DescriptorType::CombinedImageSampler as usize,
                index
            );

            vk1_0::DescriptorType::COMBINED_IMAGE_SAMPLER
        }
        2 => {
            debug_assert_eq!(DescriptorType::SampledImage as usize, index);

            vk1_0::DescriptorType::SAMPLED_IMAGE
        }
        3 => {
            debug_assert_eq!(DescriptorType::StorageImage as usize, index);

            vk1_0::DescriptorType::STORAGE_IMAGE
        }
        4 => {
            debug_assert_eq!(
                DescriptorType::UniformTexelBuffer as usize,
                index
            );

            vk1_0::DescriptorType::UNIFORM_TEXEL_BUFFER
        }
        5 => {
            debug_assert_eq!(
                DescriptorType::StorageTexelBuffer as usize,
                index
            );

            vk1_0::DescriptorType::STORAGE_TEXEL_BUFFER
        }
        6 => {
            debug_assert_eq!(DescriptorType::UniformBuffer as usize, index);

            vk1_0::DescriptorType::UNIFORM_BUFFER
        }
        7 => {
            debug_assert_eq!(DescriptorType::StorageBuffer as usize, index);

            vk1_0::DescriptorType::STORAGE_BUFFER
        }
        8 => {
            debug_assert_eq!(
                DescriptorType::UniformBufferDynamic as usize,
                index
            );

            vk1_0::DescriptorType::UNIFORM_BUFFER_DYNAMIC
        }
        9 => {
            debug_assert_eq!(
                DescriptorType::StorageBufferDynamic as usize,
                index
            );

            vk1_0::DescriptorType::STORAGE_BUFFER_DYNAMIC
        }
        10 => {
            debug_assert_eq!(DescriptorType::InputAttachment as usize, index);

            vk1_0::DescriptorType::INPUT_ATTACHMENT
        }
        11 => {
            debug_assert_eq!(
                DescriptorType::AccelerationStructure as usize,
                index
            );

            vk1_0::DescriptorType::ACCELERATION_STRUCTURE_KHR
        }
        _ => unreachable!(),
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorSizesBuilder {
    sizes: [u32; DESCRIPTOR_TYPES_COUNT],
}

impl DescriptorSizesBuilder {
    /// Create new instance without descriptors.
    pub fn zero() -> Self {
        DescriptorSizesBuilder {
            sizes: [0; DESCRIPTOR_TYPES_COUNT],
        }
    }

    /// Add a single layout binding.
    /// Useful when created with `DescriptorSizes::zero()`.
    pub fn add_binding(&mut self, binding: &DescriptorSetLayoutBinding) {
        self.sizes[binding.ty as usize] += binding.count;
    }

    /// Calculate ranges from bindings.
    pub fn from_bindings(bindings: &[DescriptorSetLayoutBinding]) -> Self {
        let mut ranges = Self::zero();

        for binding in bindings {
            ranges.add_binding(binding);
        }

        ranges
    }

    pub fn build(self) -> DescriptorSizes {
        let mut sizes = [vk1_0::DescriptorPoolSize::default()
            .builder()
            ._type(vk1_0::DescriptorType::SAMPLER)
            .descriptor_count(0);
            DESCRIPTOR_TYPES_COUNT];

        let mut count = 0u8;

        for (i, size) in self.sizes.iter().copied().enumerate() {
            if size > 0 {
                sizes[count as usize]._type = descriptor_type_from_index(i);

                sizes[count as usize].descriptor_count = size;

                count += 1;
            }
        }

        DescriptorSizes { sizes, count }
    }
}

/// Number of descriptors per type.
#[derive(Clone, Debug)]
pub struct DescriptorSizes {
    sizes: [vk1_0::DescriptorPoolSizeBuilder<'static>; DESCRIPTOR_TYPES_COUNT],
    count: u8,
}

impl DescriptorSizes {
    pub fn as_slice(&self) -> &[vk1_0::DescriptorPoolSizeBuilder<'static>] {
        &self.sizes[..self.count.into()]
    }

    /// Calculate ranges from bindings.
    pub fn from_bindings(bindings: &[DescriptorSetLayoutBinding]) -> Self {
        DescriptorSizesBuilder::from_bindings(bindings).build()
    }
}

impl Deref for DescriptorSizes {
    type Target = [vk1_0::DescriptorPoolSizeBuilder<'static>];

    fn deref(&self) -> &[vk1_0::DescriptorPoolSizeBuilder<'static>] {
        self.as_slice()
    }
}

impl Hash for DescriptorSizes {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        for size in self.as_slice() {
            hasher.write_u32(size.descriptor_count);
        }
    }
}

impl PartialEq for DescriptorSizes {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_slice().iter().zip(rhs.as_slice()).all(|(l, r)| {
            l._type == r._type && l.descriptor_count == r.descriptor_count
        })
    }
}

impl Eq for DescriptorSizes {}

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
    pub struct DescriptorSetLayout {
        pub info: DescriptorSetLayoutInfo,
        handle: vk1_0::DescriptorSetLayout,
        sizes: DescriptorSizes,
    }
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

mod layout;

pub use self::layout::*;

use crate::{
    accel::AccelerationStructure, buffer::Buffer, image::Layout,
    sampler::Sampler, view::ImageView,
};
use erupt::vk1_0;

define_handle! {
    /// Set of descriptors with specific layout.
    pub struct DescriptorSet {
        pub info: DescriptorSetInfo,
        handle: vk1_0::DescriptorSet,
        pool: vk1_0::DescriptorPool,
        pool_index: usize,
    }
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

mod layout;

pub use {self::layout::*, crate::backend::DescriptorSet};

use crate::{
    accel::AccelerationStructure, buffer::Buffer, image::Layout,
    sampler::Sampler, view::ImageView,
};

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

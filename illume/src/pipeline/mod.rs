mod compute;
mod graphics;
mod ray_tracing;

pub use self::{compute::*, graphics::*, ray_tracing::*};

use {crate::descriptor::DescriptorSetLayout, erupt::vk1_0};

define_handle! {
    /// Resource that describes layout of a pipeline.
    pub struct PipelineLayout {
        pub info: PipelineLayoutInfo,
        handle: vk1_0::PipelineLayout,
    }
}

/// Defines layouts of all descriptor sets used with pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelineLayoutInfo {
    /// Array of descriptor set layouts.
    pub sets: Vec<DescriptorSetLayout>,
}

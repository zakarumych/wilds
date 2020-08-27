mod compute;
mod graphics;
mod ray_tracing;

pub use self::{compute::*, graphics::*, ray_tracing::*};

use {
    crate::{descriptor::DescriptorSetLayout, shader::ShaderStageFlags},
    erupt::vk1_0,
};

define_handle! {
    /// Resource that describes layout of a pipeline.
    pub struct PipelineLayout {
        pub info: PipelineLayoutInfo,
        handle: vk1_0::PipelineLayout,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PushConstant {
    pub stages: ShaderStageFlags,
    pub offset: u32,
    pub size: u32,
}

/// Defines layouts of all descriptor sets used with pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelineLayoutInfo {
    /// Array of descriptor set layouts.
    pub sets: Vec<DescriptorSetLayout>,
    pub push_constants: Vec<PushConstant>,
}

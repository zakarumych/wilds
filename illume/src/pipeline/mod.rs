mod compute;
mod graphics;
mod ray_tracing;

pub use self::{compute::*, graphics::*, ray_tracing::*};

use crate::{
    descriptor::DescriptorSetLayout,
    resource::{Handle, ResourceTrait},
};

define_handle! {
    /// Resource that describes layout of a pipeline.
    pub struct PipelineLayout(PipelineLayoutInfo);
}

/// Defines layouts of all descriptor sets used with pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelineLayoutInfo {
    /// Array of descriptor set layouts.
    pub sets: Vec<DescriptorSetLayout>,
}

mod compute;
mod graphics;
mod ray_tracing;

pub use self::{compute::*, graphics::*, ray_tracing::*};

use crate::{
    descriptor::DescriptorSetLayout,
    resource::{Handle, ResourceTrait},
};

/// Resource that describes layout of a pipeline.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[repr(transparent)]

pub struct PipelineLayout {
    handle: Handle<Self>,
}

impl ResourceTrait for PipelineLayout {
    type Info = PipelineLayoutInfo;

    fn from_handle(handle: Handle<Self>) -> Self {
        Self { handle }
    }

    fn handle(&self) -> &Handle<Self> {
        &self.handle
    }
}

/// Defines layouts of all descriptor sets used with pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]

pub struct PipelineLayoutInfo {
    /// Array of descriptor set layouts.
    pub sets: Vec<DescriptorSetLayout>,
}

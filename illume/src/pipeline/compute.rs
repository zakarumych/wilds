pub use crate::backend::ComputePipeline;
use {super::PipelineLayout, crate::shader::ComputeShader};

/// Compute pipeline state definition.
#[derive(Debug)]
pub struct ComputePipelineInfo {
    /// Compute shader for the pipeline.
    pub shader: ComputeShader,

    /// Pipeline layout.
    pub layout: PipelineLayout,
}

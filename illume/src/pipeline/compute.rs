use {super::PipelineLayout, crate::shader::ComputeShader, erupt::vk1_0};

define_handle! {
    /// Resource that describes whole compute pipeline state.
    pub struct ComputePipeline {
        pub info: ComputePipelineInfo,
        handle: vk1_0::Pipeline,
    }
}

/// Compute pipeline state definition.
#[derive(Debug)]
pub struct ComputePipelineInfo {
    /// Compute shader for the pipeline.
    pub shader: ComputeShader,

    /// Pipeline layout.
    pub layout: PipelineLayout,
}

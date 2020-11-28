
use {
    super::Pass,
    crate::renderer::Context,
    bumpalo::{collections::Vec as BVec, Bump},
    color_eyre::Report,
    hecs::World,
    illume::*,
    smallvec::smallvec,
};

pub struct Input {
    target: Image,
}

pub struct Output;


pub struct RasterPass {
    render_pass: RenderPass,
    descriptor_set_layouts: DescriptorSetLayout,
    pipeline_layout: PipelineLayout,
    default_pipeline: GraphicsPipeline,
}

impl Pass<'a> for RasterPass {
    type Input = Input;
    type Output = Output;

    fn draw(
        &mut self,
        input: Input,
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<Output, Report> {

    }
}
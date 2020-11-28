use {
    super::Pipeline,
    crate::{
        camera::Camera,
        renderer::{
            pass::{
                Pass as _,
            },
            AccelerationStructure, Buffer, Context, Extent2d, Fence, Image,
            Mesh, PipelineStageFlags, Semaphore,
        },
        scene::Global3,
    },
    bumpalo::Bump,
    eyre::Report,
    hecs::World,
    std::collections::HashMap,
};


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GraphicsPipelineId(u64);


pub struct RasterPipeline {

}

impl Pipeline for RasterPipeline {
    fn draw(
        &mut self,
        target: Image,
        target_wait: &Semaphore,
        target_signal: &Semaphore,
        blases: &HashMap<Mesh, AccelerationStructure>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<(), Report> {
        self.pass.draw()
    }
}



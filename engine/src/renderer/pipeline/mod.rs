mod path_trace;
mod ray_probe;

use {
    super::{AccelerationStructure, Context, Image, Mesh, Semaphore},
    bumpalo::Bump,
    eyre::Report,
    hecs::World,
    std::collections::HashMap,
};

pub use self::{path_trace::*, ray_probe::*};

/// Pipeline represents particular rendering strategy.
/// For example path-tracing pipeline uses path tracing and denoising to render final image.
pub trait Pipeline {
    fn draw(
        &mut self,
        target: Image,
        target_wait: &Semaphore,
        target_signal: &Semaphore,
        blases: &HashMap<Mesh, AccelerationStructure>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<(), Report>;
}

pub mod combine;
pub mod diffuse_filter;
pub mod rt_prepass;

pub use self::{
    combine::CombinePass, diffuse_filter::DiffuseFilter, rt_prepass::RtPrepass,
};

use {
    crate::{clocks::ClockIndex, renderer::Context},
    bumpalo::Bump,
    color_eyre::Report,
    hecs::World,
    illume::{Fence, PipelineStageFlags, Semaphore},
};

pub trait Pass<'a> {
    type Output;
    type Input;

    fn draw(
        &mut self,
        input: Self::Input,
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<Self::Output, Report>;
}

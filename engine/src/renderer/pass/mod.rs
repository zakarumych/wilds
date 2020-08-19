pub mod atrous;
pub mod combine;
pub mod gauss_filter;
pub mod pose;
pub mod rt_prepass;

pub use self::{
    atrous::ATrousFilter, combine::CombinePass, gauss_filter::GaussFilter,
    rt_prepass::RtPrepass,
};

use {
    crate::renderer::Context,
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
        bump: &Bump,
    ) -> Result<Self::Output, Report>;
}

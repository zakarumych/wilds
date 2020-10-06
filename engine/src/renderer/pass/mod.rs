pub mod atrous;
pub mod combine;
pub mod gauss_filter;
pub mod pose;
pub mod ray_probe;
pub mod rt_prepass;

pub use self::{
    atrous::ATrousFilter, combine::CombinePass, gauss_filter::GaussFilter,
    pose::PosePass, ray_probe::RayProbe, rt_prepass::RtPrepass,
};

use {
    crate::renderer::Context,
    bumpalo::Bump,
    color_eyre::Report,
    fastbitset::BoxedBitSet,
    hecs::World,
    illume::{Fence, PipelineStageFlags, Semaphore},
    std::{
        collections::hash_map::{Entry, HashMap},
        hash::Hash,
    },
};

pub trait Pass<'a> {
    type Input;
    type Output;

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

struct SparseDescriptors<T> {
    resources: HashMap<T, u32>,
    bitset: BoxedBitSet,
    next: u32,
}

impl<T> SparseDescriptors<T>
where
    T: Hash + Eq,
{
    fn new() -> Self {
        SparseDescriptors {
            resources: HashMap::new(),
            bitset: BoxedBitSet::new(),
            next: 0,
        }
    }

    fn index(&mut self, resource: T) -> (u32, bool) {
        match self.resources.entry(resource) {
            Entry::Occupied(entry) => (*entry.get(), false),
            Entry::Vacant(entry) => {
                if let Some(index) = self.bitset.find_set() {
                    self.bitset.unset(index);
                    (*entry.insert(index as u32), true)
                } else {
                    self.next += 1;
                    (*entry.insert(self.next - 1), true)
                }
            }
        }
    }

    fn _remove(&mut self, resource: &T) -> Option<u32> {
        if let Some(value) = self.resources.remove(resource) {
            if value == self.next - 1 {
                self.next -= 1;
            } else {
                self.bitset.set(value);
            }
            Some(value)
        } else {
            None
        }
    }
}

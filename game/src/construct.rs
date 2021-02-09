use {
    hecs::{Entity, World},
    wilds::{
        assets::{Asset, Prefab, SimpleAsset},
        renderer::Renderable,
        resources::Resources,
    },
};

#[derive(Clone)]
pub struct ConstructAsset {
    renderables: Vec<Renderable>,
}

impl SimpleAsset for ConstructAsset {}

pub struct Construct;

impl Prefab for Construct {
    type Asset = ConstructAsset;

    fn spawn(
        asset: ConstructAsset,
        world: &mut World,
        resources: &mut Resources,
        entity: Entity,
    ) {
    }
}

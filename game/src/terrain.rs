use {
    hecs::Entity,
    wilds::{
        assets::{Terrain, TerrainFormat},
        engine::Engine,
        scene::Global3,
    },
};

pub fn spawn_terrain(engine: &mut Engine) -> Entity {
    let terrain = engine.load_prefab_with_format::<Terrain, _>(
        "terrain/island.ron".into(),
        TerrainFormat {
            raster: false,
            blas: true,
        },
    );

    engine.world.insert(terrain, (Global3::identity(),));
    terrain
}

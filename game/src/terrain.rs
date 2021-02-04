use wilds::{
    assets::{Terrain, TerrainFormat},
    engine::Engine,
    scene::Global3,
};

pub fn spawn_terrain(engine: &mut Engine) {
    engine.load_prefab_with_format::<Terrain, _>(
        engine.create_entity(),
        "terrain/island.ron".into(),
        Global3::from_scale(1.0),
        TerrainFormat {
            raster: false,
            blas: true,
        },
    );
}

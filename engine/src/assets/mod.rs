mod gltf;
mod terrain;

pub use self::{gltf::*, terrain::*};

use {
    crate::engine::Engine,
    eyre::WrapErr as _,
    goods::{Asset, Cache, Format},
    hecs::{Entity, World},
};

pub type Assets = Cache<String>;

pub trait Prefab: Asset + Clone {
    type Info: Send + 'static;

    /// Spawns this prefab into world.
    fn spawn(self, info: Self::Info, world: &mut World, entity: Entity);

    /// Loads asset and queue it for spawning it.
    /// Retuns `Entity` that will be supplied to `spawn` method after asset is
    /// loaded.
    /// If asset loading fails that `Entity` will be despawned.
    fn load(
        engine: &Engine,
        key: String,
        format: impl Format<Self, String>,
        info: Self::Info,
    ) -> Entity {
        let handle = engine.assets.load_with_format(key.clone(), format);
        engine.load_prefab(
            async move {
                handle.await.wrap_err_with(|| {
                    format!("Failed to load prefab '{}'", key)
                })
            },
            info,
        )
    }
}

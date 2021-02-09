mod generator;
mod gltf;
mod image;
mod material;
mod terrain;

pub use {
    self::{gltf::*, image::*, material::*, terrain::*},
    crate::resources::Resources,
    goods::*,
};

use {
    hecs::{Entity, World},
    std::{path::Path, sync::Arc},
};

pub type AssetKey = Arc<str>;
pub type Assets = Cache<AssetKey>;

pub trait Prefab {
    type Asset: Asset + Send + Sync;

    /// Spawns this prefab into world.
    fn spawn(
        asset: Self::Asset,
        world: &mut World,
        resources: &mut Resources,
        entity: Entity,
    );
}

/// Append string to asset key.
/// If string is url it is used as-is,
/// otherwise key and string are treated as `Path`s and are joined.
fn append_key(key: &AssetKey, string: &str) -> AssetKey {
    match url::Url::parse(string) {
        Ok(url) => Arc::from(url.as_str()),
        Err(_) => match Path::new(&**key).parent() {
            Some(parent) => Arc::from(parent.join(string).to_str().unwrap()),
            None => Arc::from(string),
        },
    }
}

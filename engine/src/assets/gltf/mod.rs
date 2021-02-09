mod image;
mod material;
mod prefab;
mod primitive;
mod sampler;
mod skin;
mod texture;

use {
    self::{
        image::load_gltf_image, material::load_gltf_material,
        primitive::load_gltf_primitive, sampler::load_gltf_sampler,
        texture::load_gltf_texture,
    },
    super::{append_key, image::ImageAsset, AssetKey, Assets, Format},
    crate::renderer::{Context, Renderable},
    ::image::ImageError,
    futures::future::{try_join_all, BoxFuture},
    gltf::accessor::{DataType, Dimensions},
    goods::SyncAsset,
    illume::{BufferUsage, ImageInfo, ImageView, OutOfMemory},
    std::{collections::HashMap, sync::Arc},
};

pub use prefab::Gltf;

#[derive(Clone, Copy, Debug)]
pub struct GltfFormat {
    pub mesh_vertices_usage: BufferUsage,
    pub mesh_indices_usage: BufferUsage,
}

impl GltfFormat {
    pub fn for_raster() -> Self {
        GltfFormat {
            mesh_indices_usage: BufferUsage::INDEX,
            mesh_vertices_usage: BufferUsage::VERTEX,
        }
    }

    pub fn for_raytracing() -> Self {
        GltfFormat {
            mesh_indices_usage: BufferUsage::STORAGE
                | BufferUsage::DEVICE_ADDRESS,
            mesh_vertices_usage: BufferUsage::STORAGE
                | BufferUsage::DEVICE_ADDRESS,
        }
    }
}

/// gltf scenes with initialized resources.
#[derive(Clone, Debug)]
pub struct GltfAsset {
    gltf: gltf::Gltf,
    renderables: Arc<[Box<[Renderable]>]>,
}

impl SyncAsset for GltfAsset {
    type Context = Context;
    type Repr = GltfRepr;

    fn build(repr: Self::Repr, ctx: &mut Self::Context) -> eyre::Result<Self> {
        let images = repr
            .gltf
            .images()
            .map(|image| load_gltf_image(&repr, image, ctx))
            .collect::<Result<Vec<_>, _>>()?;

        let samplers = repr
            .gltf
            .samplers()
            .map(|sampler| load_gltf_sampler(sampler, ctx))
            .collect::<Result<Vec<_>, _>>()?;

        let mut default_sampler = None;

        let textures = repr
            .gltf
            .textures()
            .map(|texture| {
                load_gltf_texture(
                    texture,
                    &images,
                    &samplers,
                    &mut default_sampler,
                    ctx,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let materials = repr
            .gltf
            .materials()
            .map(|material| load_gltf_material(material, &textures))
            .collect::<Result<Vec<_>, _>>()?;

        let renderables = repr
            .gltf
            .meshes()
            .map(|mesh| {
                mesh.primitives()
                    .map(|prim| {
                        load_gltf_primitive(&repr, prim, &materials, ctx)
                    })
                    .collect::<Result<_, _>>()
            })
            .collect::<Result<_, _>>()?;

        Ok(GltfAsset {
            gltf: repr.gltf,
            renderables,
        })
    }
}

/// Intermediate gltf representation.
/// Contains parsed gltf tree and all sources loaded.
pub struct GltfRepr {
    gltf: gltf::Gltf,
    buffers: HashMap<String, Box<[u8]>>,
    images: HashMap<String, ImageView>,
    config: GltfFormat,
}

impl Format<GltfRepr, AssetKey> for GltfFormat {
    type DecodeFuture = BoxFuture<'static, eyre::Result<GltfRepr>>;

    fn decode(
        self,
        key: AssetKey,
        bytes: Box<[u8]>,
        assets: &Assets,
    ) -> BoxFuture<'static, eyre::Result<GltfRepr>> {
        match gltf::Gltf::from_slice(&bytes) {
            Err(err) => Box::pin(async move { Err(err.into()) }),
            Ok(gltf) => {
                if gltf.scenes().len() == 0 {
                    return Box::pin(async {
                        Err(GltfLoadingError::NoScenes.into())
                    });
                }

                let buffers =
                    try_join_all(gltf.buffers().filter_map(
                        |b| match b.source() {
                            gltf::buffer::Source::Bin => None,
                            gltf::buffer::Source::Uri(uri) => {
                                Some(assets.read(append_key(&key, uri)))
                            }
                        },
                    ));

                let images =
                    try_join_all(gltf.images().filter_map(
                        |b| match b.source() {
                            gltf::image::Source::View { .. } => None,
                            gltf::image::Source::Uri { uri, .. } => {
                                Some(
                                    assets.load::<ImageAsset>(append_key(
                                        &key, uri,
                                    )),
                                )
                            }
                        },
                    ));

                Box::pin(async move {
                    // let (buffers, images) = try_join!(buffers, images)?;
                    let (buffers, images) = (buffers.await?, images.await?);

                    let buffers_uri =
                        gltf.buffers().filter_map(|b| match b.source() {
                            gltf::buffer::Source::Bin => None,
                            gltf::buffer::Source::Uri(uri) => {
                                Some(uri.to_owned())
                            }
                        });

                    let images_uri =
                        gltf.images().filter_map(|b| match b.source() {
                            gltf::image::Source::View { .. } => None,
                            gltf::image::Source::Uri { uri, .. } => {
                                Some(uri.to_owned())
                            }
                        });

                    Ok(GltfRepr {
                        buffers: buffers_uri.zip(buffers).collect(),
                        images: images_uri
                            .zip(images)
                            .map(|(uri, texture)| (uri, texture.into_inner()))
                            .collect(),
                        config: self,
                        gltf,
                    })
                })
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GltfLoadingError {
    #[error(transparent)]
    GltfError {
        #[from]
        source: gltf::Error,
    },

    #[error("GLTF with no scenes")]
    NoScenes,

    #[error(transparent)]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Accessor has unexpected dimensions `{unexpected:?}`. Expected `{expected:?}`")]
    UnexpectedDimensions {
        unexpected: Dimensions,
        expected: &'static [Dimensions],
    },

    #[error("Accessor has unexpected data type `{unexpected:?}`. Expected `{expected:?}`")]
    UnexpectedDataType {
        unexpected: DataType,
        expected: &'static [DataType],
    },

    #[error("Sparse accessors are not supported")]
    SparseAccessorUnsupported,

    #[error("Accessor does not fit the view")]
    AccessorOutOfBound,

    #[error("View does not fit the source")]
    ViewOutOfBound,

    #[error("Source does not exist")]
    MissingSource,

    #[error("Unsupported mesh without position attribute")]
    MissingPositionAttribute,

    #[error("Texture referenced in material not found in textures array")]
    MissingTexture,

    #[error("Unsupported mesh topology")]
    UnsupportedTopology { unsupported: gltf::mesh::Mode },

    #[error("Failed to load image data: `{source}`")]
    ImageError {
        #[from]
        source: ImageError,
    },

    #[error("Combination paramters `{info:?}` is unsupported")]
    UnsupportedImage { info: ImageInfo },
}

fn align_vec(bytes: &mut Vec<u8>, align_mask: usize) {
    let new_size = (bytes.len() + align_mask) & !align_mask;
    bytes.resize(new_size, 0xfe);
}

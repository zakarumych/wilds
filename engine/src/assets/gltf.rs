use {
    crate::renderer::{
        Binding, Context, FromBytes as _, Indices, Material, Mesh, MeshBuilder,
        Normal3d, Position3d, PositionNormalTangent3dUV, Tangent3d, Texture,
        VertexType, UV,
    },
    byteorder::LittleEndian,
    futures::future::{try_join_all, BoxFuture},
    illume::{
        BorderColor, BufferInfo, BufferUsage, CreateImageError, Filter, Format,
        ImageExtent, ImageInfo, ImageUsage, ImageViewInfo, IndexType,
        MemoryUsageFlags, MipmapMode, OutOfMemory, PrimitiveTopology, Sampler,
        SamplerAddressMode, SamplerInfo, Samples1,
    },
    std::{
        collections::HashMap,
        convert::{TryFrom as _, TryInto as _},
        error::Error,
        mem::{align_of, size_of},
        ops::Range,
        str::FromStr,
        sync::Arc,
    },
    ultraviolet::Mat4,
};

#[derive(Clone, Debug)]
pub struct GltfMesh {
    pub primitives: Range<usize>,
    pub materials: Vec<Material>,
}

#[derive(Clone, Debug)]
pub struct GltfNode {
    pub transform: Mat4,
    pub children: Box<[usize]>,
    pub mesh: Option<GltfMesh>,
}

#[derive(Clone, Debug)]
pub struct GltfScene {
    pub nodes: Box<[usize]>,
}

#[derive(Clone, Debug)]
pub struct Gltf {
    pub scenes: Arc<[GltfScene]>,
    pub scene: Option<usize>,
    pub nodes: Arc<[GltfNode]>,
    pub meshes: Arc<[Mesh]>,
}

#[derive(Clone, Debug)]
pub struct GltfRepr {
    gltf: gltf::Gltf,
    sources: HashMap<String, Arc<[u8]>>,
    usage: BufferUsage,
}

#[derive(Debug, thiserror::Error)]
pub enum GltfError {
    #[error("{source}")]
    Gltf {
        #[from]
        source: gltf::Error,
    },

    #[error("Out of device memory")]
    OutOfMemory,

    #[error("Unsupported feature")]
    Unsupported,

    #[error("GLTF contains invalid view")]
    InvalidView,

    #[error("GLTF contains invalid accessor")]
    InvalidAccessor,

    #[error("Failed to load external binary data source")]
    SourceLoadingFailed,

    #[error("Failed to load texture: {source}")]
    ImageDecode {
        #[from]
        source: image::ImageError,
    },

    #[error("Combination paramters `{info:?}` is unsupported")]
    UnsupportedImage { info: ImageInfo },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
}

impl From<OutOfMemory> for GltfError {
    fn from(_: OutOfMemory) -> Self {
        GltfError::OutOfMemory
    }
}

impl From<CreateImageError> for GltfError {
    fn from(err: CreateImageError) -> Self {
        match err {
            CreateImageError::OutOfMemory { .. } => GltfError::OutOfMemory,
            CreateImageError::Unsupported { info } => {
                GltfError::UnsupportedImage { info }
            }
            CreateImageError::Other { source } => GltfError::Other { source },
        }
    }
}

impl goods::SyncAsset for Gltf {
    type Context = Context;
    type Error = GltfError;
    type Repr = GltfRepr;

    fn build(repr: GltfRepr, context: &mut Context) -> Result<Self, GltfError> {
        let mut total_polygons = 0;
        let mut total_data = Vec::new();

        struct MeshAux {
            vertices: Range<usize>,
            indices: Option<IndicesAux>,
            count: u32,
            vertex_count: u32,
            topology: PrimitiveTopology,
        }

        let mut primitive_ranges: Vec<Range<usize>> = Vec::new();
        let meshes_aux: Vec<MeshAux> = repr
            .gltf
            .document
            .meshes()
            .flat_map(|mesh| {
                let offset =
                    primitive_ranges.last().map(|range| range.end).unwrap_or(0);
                let size = mesh.primitives().len();
                primitive_ranges.push(offset..offset + size);

                mesh.primitives()
                    .map(|primitive| -> Result<_, GltfError> {
                        align_vec(
                            &mut total_data,
                            15 | (align_of::<u32>() - 1),
                        );

                        let (vertices, vertex_count) = load_vertices(
                            primitive.clone(), // Could be copy.
                            repr.gltf.blob.as_deref(),
                            &repr.sources,
                            &mut total_data,
                        )?;

                        tracing::warn!("{} vertices loaded", vertex_count);

                        let mut count = vertex_count;
                        let indices = primitive
                            .indices()
                            .map(|indices| {
                                total_polygons += indices.count();
                                count = indices.count();

                                align_vec(
                                    &mut total_data,
                                    15 | (align_of::<u32>() - 1),
                                );

                                load_indices(
                                    &indices,
                                    repr.gltf.blob.as_deref(),
                                    &repr.sources,
                                    &mut total_data,
                                )
                            })
                            .transpose()?;

                        Ok(MeshAux {
                            vertices,
                            indices,
                            vertex_count: vertex_count
                                .try_into()
                                .map_err(|_| GltfError::OutOfMemory)?,
                            count: count
                                .try_into()
                                .map_err(|_| GltfError::OutOfMemory)?,
                            topology: match primitive.mode() {
                                gltf::mesh::Mode::Points => {
                                    PrimitiveTopology::PointList
                                }
                                gltf::mesh::Mode::Lines => {
                                    PrimitiveTopology::LineList
                                }
                                gltf::mesh::Mode::LineLoop => {
                                    return Err(GltfError::Unsupported);
                                }
                                gltf::mesh::Mode::LineStrip => {
                                    PrimitiveTopology::LineStrip
                                }
                                gltf::mesh::Mode::Triangles => {
                                    PrimitiveTopology::TriangleList
                                }
                                gltf::mesh::Mode::TriangleStrip => {
                                    PrimitiveTopology::TriangleStrip
                                }
                                gltf::mesh::Mode::TriangleFan => {
                                    PrimitiveTopology::TriangleFan
                                }
                            },
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Result<_, _>>()?;

        let buffer = context.create_buffer_static(
            BufferInfo {
                align: 255,
                size: u64::try_from(total_data.len())
                    .map_err(|_| GltfError::OutOfMemory)?,
                usage: repr.usage,
                memory: MemoryUsageFlags::empty(),
            },
            &total_data,
        )?;

        let meshes = meshes_aux
            .into_iter()
            .map(|mesh| {
                let mut bindings = Vec::new();

                bindings.push(Binding {
                    buffer: buffer.clone(),
                    offset: mesh.vertices.start as u64,
                    layout: PositionNormalTangent3dUV::layout(),
                });

                MeshBuilder {
                    bindings,
                    indices: match mesh.indices {
                        None => None,
                        Some(IndicesAux::U16(range)) => Some(Indices {
                            buffer: buffer.clone(),
                            offset: range.start as u64,
                            index_type: IndexType::U16,
                        }),
                        Some(IndicesAux::U32(range)) => Some(Indices {
                            buffer: buffer.clone(),
                            offset: range.start as u64,
                            index_type: IndexType::U32,
                        }),
                    },
                    topology: mesh.topology,
                }
                .build(mesh.count, mesh.vertex_count)
            })
            .collect();

        let images: Vec<_> = repr
            .gltf
            .images()
            .map(|image| -> Result<_, GltfError> {
                let data = match image.source() {
                    gltf::image::Source::View { view, .. } => {
                        let range =
                            view.offset()..view.offset() + view.length();
                        match view.buffer().source() {
                            gltf::buffer::Source::Bin => {
                                &repr.gltf.blob.as_ref().unwrap()[range]
                            }
                            gltf::buffer::Source::Uri(uri) => {
                                &repr.sources[uri][range]
                            }
                        }
                    }
                    gltf::image::Source::Uri { uri, .. } => {
                        &repr.sources[uri][..]
                    }
                };
                let image = image::load_from_memory(data)?.to_rgba();
                let image = context.create_image_static(
                    ImageInfo {
                        extent: ImageExtent::D2 {
                            width: image.dimensions().0,
                            height: image.dimensions().1,
                        },
                        format: Format::RGBA8Unorm,
                        levels: 1,
                        layers: 1,
                        samples: Samples1,
                        usage: ImageUsage::SAMPLED,
                        memory: MemoryUsageFlags::empty(),
                    },
                    0,
                    0,
                    &image.into_raw(),
                )?;

                let image = context
                    .device
                    .create_image_view(ImageViewInfo::new(image))?;
                Ok(image)
            })
            .collect::<Result<_, _>>()?;

        let samplers: Vec<_> = repr
            .gltf
            .samplers()
            .map(|sampler| {
                context.create_sampler(SamplerInfo {
                        mag_filter: match sampler.mag_filter() {
                            None | Some(gltf::texture::MagFilter::Nearest) => {
                                Filter::Nearest
                            }
                            Some(gltf::texture::MagFilter::Linear) => {
                                Filter::Linear
                            }
                        },
                        min_filter: match sampler.min_filter() {
                            None
                            | Some(gltf::texture::MinFilter::Nearest)
                            | Some(
                                gltf::texture::MinFilter::NearestMipmapNearest,
                            )
                            | Some(
                                gltf::texture::MinFilter::NearestMipmapLinear,
                            ) => Filter::Nearest,
                            _ => Filter::Linear,
                        },
                        mipmap_mode: match sampler.min_filter() {
                            None
                            | Some(gltf::texture::MinFilter::Nearest)
                            | Some(gltf::texture::MinFilter::Linear)
                            | Some(
                                gltf::texture::MinFilter::NearestMipmapNearest,
                            )
                            | Some(
                                gltf::texture::MinFilter::LinearMipmapNearest,
                            ) => MipmapMode::Nearest,
                            _ => MipmapMode::Linear,
                        },
                        address_mode_u: match sampler.wrap_s() {
                            gltf::texture::WrappingMode::ClampToEdge => {
                                SamplerAddressMode::ClampToEdge
                            }
                            gltf::texture::WrappingMode::MirroredRepeat => {
                                SamplerAddressMode::MirroredRepeat
                            }
                            gltf::texture::WrappingMode::Repeat => {
                                SamplerAddressMode::Repeat
                            }
                        },
                        address_mode_v: match sampler.wrap_t() {
                            gltf::texture::WrappingMode::ClampToEdge => {
                                SamplerAddressMode::ClampToEdge
                            }
                            gltf::texture::WrappingMode::MirroredRepeat => {
                                SamplerAddressMode::MirroredRepeat
                            }
                            gltf::texture::WrappingMode::Repeat => {
                                SamplerAddressMode::Repeat
                            }
                        },
                        address_mode_w: SamplerAddressMode::Repeat,
                        mip_lod_bias: 0.0.into(),
                        max_anisotropy: None,
                        compare_op: None,
                        min_lod: 0.0.into(),
                        max_lod: 100.0.into(),
                        border_color: BorderColor::FloatTransparentBlack,
                        unnormalized_coordinates: false,
                    })
            })
            .collect::<Result<_, _>>()?;

        let mut default_sampler: Option<Sampler> = None;

        let materials: Vec<_> = repr
            .gltf
            .materials()
            .map(|material| -> Result<_, OutOfMemory> {
                let pbr = material.pbr_metallic_roughness();
                Ok(Material {
                    albedo: pbr
                        .base_color_texture()
                        .map(|info| -> Result<_, OutOfMemory> {
                            let texture = info.texture();
                            let image =
                                images[texture.source().index()].clone();
                            let sampler = match texture.sampler().index() {
                                Some(index) => samplers[index].clone(),
                                None => match &mut default_sampler {
                                    Some(default_sampler) => {
                                        default_sampler.clone()
                                    }
                                    None => {
                                        let sampler = context.create_sampler(
                                            SamplerInfo::default(),
                                        )?;
                                        default_sampler = Some(sampler.clone());
                                        sampler
                                    }
                                },
                            };
                            Ok(Texture { image, sampler })
                        })
                        .transpose()?,
                    albedo_factor: {
                        let [r, g, b, a] = pbr.base_color_factor();
                        [r.into(), g.into(), b.into(), a.into()]
                    },
                    normal: material
                        .normal_texture()
                        .map(|info| {
                            let texture = info.texture();
                            let image =
                                images[texture.source().index()].clone();
                            let sampler = match texture.sampler().index() {
                                Some(index) => samplers[index].clone(),
                                None => match &mut default_sampler {
                                    Some(default_sampler) => {
                                        default_sampler.clone()
                                    }
                                    None => {
                                        let sampler = context.create_sampler(
                                            SamplerInfo::default(),
                                        )?;
                                        default_sampler = Some(sampler.clone());
                                        sampler
                                    }
                                },
                            };
                            Ok(Texture { image, sampler })
                        })
                        .transpose()?,
                    normal_factor: material
                        .normal_texture()
                        .map(|info| info.scale())
                        .unwrap_or(0.0)
                        .into(),
                })
            })
            .collect::<Result<_, _>>()?;

        let default_material = Material {
            albedo: None,
            albedo_factor: [0.8.into(); 4],
            normal: None,
            normal_factor: 0.0.into(),
        };

        let nodes = repr
            .gltf
            .nodes()
            .map(|node| {
                let m = node.transform().matrix();
                let transform = Mat4::new(
                    m[0].into(),
                    m[1].into(),
                    m[2].into(),
                    m[3].into(),
                );

                GltfNode {
                    transform,
                    children: node
                        .children()
                        .map(|child| child.index())
                        .collect(),

                    mesh: node.mesh().map(|mesh| {
                        let materials = mesh
                            .primitives()
                            .map(|primitive| {
                                match primitive.material().index() {
                                    Some(index) => materials[index].clone(),
                                    None => default_material.clone(),
                                }
                            })
                            .collect();
                        GltfMesh {
                            primitives: primitive_ranges[mesh.index()].clone(),
                            materials,
                        }
                    }),
                }
            })
            .collect();

        let scenes = repr
            .gltf
            .scenes()
            .map(|scene| GltfScene {
                nodes: scene.nodes().map(|node| node.index()).collect(),
            })
            .collect();

        tracing::debug!("Total polygons '{}'", total_polygons);

        Ok(Gltf {
            meshes,
            nodes,
            scenes,
            scene: repr.gltf.default_scene().map(|scene| scene.index()),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GltfFormat {
    pub raster: bool,
    pub blas: bool,
}

impl<K> goods::Format<Gltf, K> for GltfFormat
where
    K: FromStr + goods::Key,
{
    type DecodeFuture = BoxFuture<'static, Result<GltfRepr, GltfError>>;
    type Error = GltfError;

    fn decode(
        self,
        bytes: Vec<u8>,
        cache: &goods::Cache<K>,
    ) -> Self::DecodeFuture {
        pub use gltf::*;

        let mut usage = BufferUsage::empty();
        if self.raster {
            usage |= BufferUsage::INDEX | BufferUsage::VERTEX;
        }
        if self.blas {
            usage |= BufferUsage::RAY_TRACING
                | BufferUsage::SHADER_DEVICE_ADDRESS
                | BufferUsage::STORAGE;
        }

        match Gltf::from_slice(&bytes) {
            Ok(gltf) => {
                let sources = try_join_all(
                    gltf.buffers()
                        .filter_map(|buffer| match buffer.source() {
                            gltf::buffer::Source::Bin => None,
                            gltf::buffer::Source::Uri(uri) => {
                                Some(uri.to_owned())
                            }
                        })
                        .chain(gltf.images().filter_map(|image| {
                            match image.source() {
                                gltf::image::Source::View { .. } => None,
                                gltf::image::Source::Uri { uri, .. } => {
                                    Some(uri.to_owned())
                                }
                            }
                        }))
                        .map(|source| {
                            let handle =
                                source.parse().ok().map(|key| cache.load(key));
                            async move {
                                let buffer = handle
                                    .ok_or(GltfError::SourceLoadingFailed)?
                                    .await
                                    .map_err(|_| {
                                        GltfError::SourceLoadingFailed
                                    })?;

                                Ok::<_, GltfError>((source, buffer))
                            }
                        }),
                );

                Box::pin(async move {
                    let sources = sources.await?.into_iter().collect();
                    Ok(GltfRepr {
                        gltf,
                        sources,
                        usage,
                    })
                })
            }
            Err(err) => {
                Box::pin(async move { Err(GltfError::Gltf { source: err }) })
            }
        }
    }
}

trait GltfVertexType: VertexType {
    const DIMENSIONS: gltf::accessor::Dimensions;
    const DATA_TYPE: gltf::accessor::DataType;
}

impl GltfVertexType for Position3d {
    const DATA_TYPE: gltf::accessor::DataType = gltf::accessor::DataType::F32;
    const DIMENSIONS: gltf::accessor::Dimensions =
        gltf::accessor::Dimensions::Vec3;
}

impl GltfVertexType for Normal3d {
    const DATA_TYPE: gltf::accessor::DataType = gltf::accessor::DataType::F32;
    const DIMENSIONS: gltf::accessor::Dimensions =
        gltf::accessor::Dimensions::Vec3;
}

impl GltfVertexType for Tangent3d {
    const DATA_TYPE: gltf::accessor::DataType = gltf::accessor::DataType::F32;
    const DIMENSIONS: gltf::accessor::Dimensions =
        gltf::accessor::Dimensions::Vec4;
}

impl GltfVertexType for UV {
    const DATA_TYPE: gltf::accessor::DataType = gltf::accessor::DataType::F32;
    const DIMENSIONS: gltf::accessor::Dimensions =
        gltf::accessor::Dimensions::Vec2;
}

#[tracing::instrument(skip(blob, sources))]
fn load_vertex_attribute<'a, V: GltfVertexType>(
    accessor: gltf::accessor::Accessor<'_>,
    blob: Option<&'a [u8]>,
    sources: &'a HashMap<String, Arc<[u8]>>,
) -> Result<impl Iterator<Item = V> + 'a, GltfError> {
    if V::DIMENSIONS != accessor.dimensions() {
        tracing::error!("Accessor to vertex attribute '{}' has wrong dimensions. Expected: {:?}, found: {:?}", V::NAME, V::DIMENSIONS, accessor.dimensions());
        return Err(GltfError::Unsupported);
    }

    if V::DATA_TYPE != accessor.data_type() {
        tracing::error!("Accessor to vertex attribute '{}' has wrong data type. Expected: {:?}, found: {:?}", V::NAME, V::DATA_TYPE, accessor.data_type());
        return Err(GltfError::Unsupported);
    }

    if size_of::<V>() != accessor.size() {
        tracing::error!("Accessor to vertex attribute '{}' has wrong size. Expected: {}, found: {}", V::NAME, size_of::<V>(), accessor.size());
        return Err(GltfError::Unsupported);
    }

    let view = accessor.view().ok_or_else(|| {
        tracing::error!("Accessor to vertex attribute '{}' is sparse. Sparse accessors are unsupported yet", V::NAME);
        GltfError::Unsupported
    })?;

    let stride = view.stride().unwrap_or(accessor.size());

    let accessor_length = if accessor.count() == 0 {
        0
    } else {
        (accessor.count() - 1) * stride + accessor.size()
    };

    if view.length() < accessor_length + accessor.offset() {
        tracing::error!(
            "Accessor to vertex attribute '{}' is out of its buffer view bounds",
            V::NAME
        );
        return Err(GltfError::InvalidAccessor);
    }

    let bytes: &[u8] = match view.buffer().source() {
        gltf::buffer::Source::Bin => blob.ok_or(GltfError::InvalidView)?,
        gltf::buffer::Source::Uri(uri) => {
            &sources.get(uri).ok_or_else(|| {
                tracing::error!(
                    "View of accessor to vertex attribute '{}' has non-existent source",
                    V::NAME
                );
                GltfError::InvalidView
            })?
        }
    };

    if bytes.len() < view.offset() + view.length() {
        tracing::error!(
            "View of accessor to vertex attribute '{}' is out of its buffer bounds",
            V::NAME
        );
        return Err(GltfError::InvalidView);
    }

    let bytes = &bytes[view.offset()..][..view.length()][accessor.offset()..];

    // glTF explicitly defines that binary data is in little-endian.
    Ok(V::from_bytes_iter::<LittleEndian>(bytes, stride))
}

enum IterOrDefaults<I, T> {
    Iter(I),
    Defaults(T),
}

fn iter_or_defaults<I, T>(iter: Option<I>, default: T) -> IterOrDefaults<I, T> {
    match iter {
        Some(iter) => IterOrDefaults::Iter(iter),
        None => IterOrDefaults::Defaults(default),
    }
}

impl<I, T> Iterator for IterOrDefaults<I, T>
where
    I: Iterator<Item = T>,
    T: Copy,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Self::Iter(iter) => iter.next(),
            Self::Defaults(value) => Some(*value),
        }
    }
}

#[tracing::instrument(skip(blob, sources, output))]
fn load_vertices(
    primitive: gltf::mesh::Primitive<'_>,
    blob: Option<&[u8]>,
    sources: &HashMap<String, Arc<[u8]>>,
    output: &mut Vec<u8>,
) -> Result<(Range<usize>, usize), GltfError> {
    let position = primitive
        .get(&gltf::Semantic::Positions)
        .ok_or(GltfError::Unsupported)?;

    let position_attribute_iter =
        load_vertex_attribute::<Position3d>(position, blob, sources)?;

    let normals_attribute_iter = primitive
        .get(&gltf::Semantic::Normals)
        .map(|accessor| {
            load_vertex_attribute::<Normal3d>(accessor, blob, sources)
        })
        .transpose()?;

    let normals_attribute_iter =
        iter_or_defaults(normals_attribute_iter, Normal3d([0.0; 3]));

    let tangents_attribute_iter = primitive
        .get(&gltf::Semantic::Tangents)
        .map(|accessor| {
            load_vertex_attribute::<Tangent3d>(accessor, blob, sources)
        })
        .transpose()?;

    let tangents_attribute_iter =
        iter_or_defaults(tangents_attribute_iter, Tangent3d([0.0; 4]));

    let uv_attribute_iter = primitive
        .get(&gltf::Semantic::TexCoords(0))
        .map(|accessor| load_vertex_attribute::<UV>(accessor, blob, sources))
        .transpose()?;

    let uv_attribute_iter = iter_or_defaults(uv_attribute_iter, UV([0.0; 2]));

    let vertex_iter = position_attribute_iter
        .zip(normals_attribute_iter)
        .zip(tangents_attribute_iter)
        .zip(uv_attribute_iter);

    let start = output.len();
    let count = vertex_iter
        .map(|(((position, normal), tangent), uv)| {
            let vertex = PositionNormalTangent3dUV {
                position,
                normal,
                tangent,
                uv,
            };
            output.extend_from_slice(bytemuck::bytes_of(&vertex));
        })
        .count();

    Ok((start..output.len(), count))
}

enum IndicesAux {
    U16(Range<usize>),
    U32(Range<usize>),
}

fn load_indices(
    accessor: &gltf::accessor::Accessor<'_>,
    blob: Option<&[u8]>,
    sources: &HashMap<String, Arc<[u8]>>,
    output: &mut Vec<u8>,
) -> Result<IndicesAux, GltfError> {
    if gltf::accessor::Dimensions::Scalar != accessor.dimensions() {
        return Err(GltfError::Unsupported);
    }

    let view = accessor.view().ok_or(GltfError::Unsupported)?;

    let stride = view.stride().unwrap_or(accessor.size());

    let accessor_length = if accessor.count() == 0 {
        0
    } else {
        (accessor.count() - 1) * stride + accessor.size()
    };

    if view.length() < accessor_length + accessor.offset() {
        return Err(GltfError::InvalidAccessor);
    }

    let bytes: &[u8] = match view.buffer().source() {
        gltf::buffer::Source::Bin => blob.ok_or(GltfError::InvalidView)?,
        gltf::buffer::Source::Uri(uri) => {
            &sources.get(uri).ok_or(GltfError::InvalidView)?
        }
    };

    if bytes.len() < view.offset() + view.length() {
        return Err(GltfError::InvalidView);
    }

    let bytes = &bytes[view.offset()..][..view.length()][accessor.offset()..];

    // glTF explicitly defines the endianness of binary data as little endian
    match accessor.data_type() {
        gltf::accessor::DataType::U16 => {
            if size_of::<u16>() != accessor.size() {
                return Err(GltfError::Unsupported);
            }

            let start = output.len();

            // #[cfg(target_endian = "little")]
            // {
            //     if stride == size_of::<u16>() {
            //         // Just copy bytes for packed indices
            //         // if endianess is the same.
            //         output.extend_from_slice(unsafe {
            //             std::slice::from_raw_parts(
            //                 bytes.as_ptr() as *const _,
            //                 bytes.len(),
            //             )
            //         });
            //         return Ok(IndicesAux::U16(start..output.len()));
            //     }
            // }

            let mut count = 0;
            for index in u16::from_bytes_iter::<LittleEndian>(bytes, stride)
                .take(accessor.count())
            {
                output.extend((index as u32).to_ne_bytes().iter().copied());
                count += 1;
            }
            assert_eq!(accessor.count(), count);

            Ok(IndicesAux::U32(start..output.len()))
        }
        gltf::accessor::DataType::U32 => {
            if size_of::<u32>() != accessor.size() {
                return Err(GltfError::Unsupported);
            }

            let start = output.len();

            #[cfg(target_endian = "little")]
            {
                if stride == size_of::<u32>() {
                    // Just copy bytes for packed indices
                    // if endianess is the same.
                    output.extend_from_slice(unsafe {
                        std::slice::from_raw_parts(
                            bytes.as_ptr() as *const _,
                            bytes.len(),
                        )
                    });
                    return Ok(IndicesAux::U32(start..output.len()));
                }
            }

            let mut count = 0;
            for index in u32::from_bytes_iter::<LittleEndian>(bytes, stride)
                .take(accessor.count())
            {
                output.extend(index.to_ne_bytes().iter().copied());
                count += 1;
            }
            assert_eq!(accessor.count(), count);

            Ok(IndicesAux::U32(start..output.len()))
        }
        _ => Err(GltfError::Unsupported),
    }
}

fn align_vec(bytes: &mut Vec<u8>, align_mask: usize) {
    let new_size = (bytes.len() + align_mask) & !align_mask;
    bytes.resize(new_size, 0xfe);
}

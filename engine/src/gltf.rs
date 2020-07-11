use {
    crate::renderer::{
        Binding, FromBytes as _, Indices, Mesh, MeshBuilder, Normal3d,
        Position3d, Position3dNormal3d, VertexLayout, VertexLocation,
        VertexType,
    },
    byteorder::LittleEndian,
    futures::future::{try_join_all, BoxFuture},
    illume::{
        Buffer, BufferInfo, BufferUsage, Device, IndexType, MemoryUsageFlags,
        OutOfMemory, PrimitiveTopology,
    },
    std::{
        collections::HashMap,
        convert::{TryFrom as _, TryInto as _},
        future::Future,
        hash::Hash,
        mem::{align_of, size_of, MaybeUninit},
        ops::Range,
        pin::Pin,
        str::FromStr,
        sync::Arc,
        task::{Context, Poll},
    },
    ultraviolet::{Bivec3, Mat4, Rotor3, Vec4},
};

#[derive(Clone, Debug)]
pub struct GltfNode {
    pub transform: Mat4,
    pub children: Box<[usize]>,
    pub mesh: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct GltfScene {
    pub nodes: Box<[usize]>,
}

#[derive(Clone, Debug)]
pub struct Gltf {
    pub scenes: Arc<[GltfScene]>,
    pub nodes: Arc<[GltfNode]>,
    pub meshes: Arc<[Box<[Mesh]>]>,
    pub scene: Option<usize>,
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
}

impl From<OutOfMemory> for GltfError {
    fn from(_: OutOfMemory) -> Self {
        GltfError::OutOfMemory
    }
}

impl goods::SyncAsset for Gltf {
    type Context = Device;
    type Error = GltfError;
    type Repr = GltfRepr;

    fn build(repr: GltfRepr, device: &mut Device) -> Result<Self, GltfError> {
        let mut total_polygons = 0;
        let mut total_data = Vec::new();

        struct MeshAux {
            // positions: Option<Range<usize>>,
            // normals: Option<Range<usize>>,
            posnorms: Range<usize>,
            indices: Option<IndicesAux>,
            count: u32,
            vertex_count: u32,
            topology: PrimitiveTopology,
        }

        let meshes: Vec<Vec<MeshAux>> = repr
            .gltf
            .document
            .meshes()
            .map(|mesh| {
                mesh.primitives()
                    .map(|primitive| -> Result<_, GltfError> {
                        // let positions = primitive
                        //     .get(&gltf::Semantic::Positions)
                        //     .map(|positions| {
                        //         align_vec(
                        //             &mut total_data,
                        //             align_of::<Position3d>() - 1,
                        //         );
                        //         Ok::<_, GltfError>((
                        //             positions.count(),
                        //             load_vertices::<Position3d>(
                        //                 &positions,
                        //                 repr.gltf
                        //                     .blob
                        //                     .as_ref()
                        //                     .map(std::ops::Deref::deref),
                        //                 &repr.sources,
                        //                 &mut total_data,
                        //             )?,
                        //         ))
                        //     })
                        //     .transpose()?;

                        // let normals = primitive
                        //     .get(&gltf::Semantic::Normals)
                        //     .map(|normals| {
                        //         align_vec(
                        //             &mut total_data,
                        //             align_of::<Normal3d>() - 1,
                        //         );
                        //         Ok::<_, GltfError>((
                        //             normals.count(),
                        //             load_vertices::<Normal3d>(
                        //                 &normals,
                        //                 repr.gltf
                        //                     .blob
                        //                     .as_ref()
                        //                     .map(std::ops::Deref::deref),
                        //                 &repr.sources,
                        //                 &mut total_data,
                        //             )?,
                        //         ))
                        //     })
                        //     .transpose()?;

                        let positions = primitive
                            .get(&gltf::Semantic::Positions)
                            .ok_or(GltfError::Unsupported)?;

                        let positions_view =
                            positions.view().ok_or(GltfError::Unsupported)?;

                        let positions_stride =
                            positions_view.stride().unwrap_or(positions.size());

                        let positions_length = if positions.count() == 0 {
                            0
                        } else {
                            (positions.count() - 1) * positions_stride
                                + positions.size()
                        };

                        if positions_view.length()
                            < positions_length + positions.offset()
                        {
                            return Err(GltfError::InvalidAccessor);
                        }

                        let positions_bytes: &[u8] =
                            match positions_view.buffer().source() {
                                gltf::buffer::Source::Bin => repr
                                    .gltf
                                    .blob
                                    .as_ref()
                                    .ok_or(GltfError::InvalidView)?,
                                gltf::buffer::Source::Uri(uri) => &repr
                                    .sources
                                    .get(uri)
                                    .ok_or(GltfError::InvalidView)?,
                            };

                        if positions_bytes.len()
                            < positions_view.offset() + positions_view.length()
                        {
                            return Err(GltfError::InvalidView);
                        }

                        let positions_bytes = &positions_bytes
                            [positions_view.offset()..]
                            [..positions_view.length()][positions.offset()..];

                        let normals = primitive
                            .get(&gltf::Semantic::Normals)
                            .ok_or(GltfError::Unsupported)?;

                        let normals_view =
                            normals.view().ok_or(GltfError::Unsupported)?;

                        let normals_stride =
                            normals_view.stride().unwrap_or(normals.size());

                        let normals_length = if normals.count() == 0 {
                            0
                        } else {
                            (normals.count() - 1) * normals_stride
                                + normals.size()
                        };

                        if normals_view.length()
                            < normals_length + normals.offset()
                        {
                            return Err(GltfError::InvalidAccessor);
                        }

                        let normals_bytes: &[u8] =
                            match normals_view.buffer().source() {
                                gltf::buffer::Source::Bin => repr
                                    .gltf
                                    .blob
                                    .as_ref()
                                    .ok_or(GltfError::InvalidView)?,
                                gltf::buffer::Source::Uri(uri) => &repr
                                    .sources
                                    .get(uri)
                                    .ok_or(GltfError::InvalidView)?,
                            };

                        if normals_bytes.len()
                            < normals_view.offset() + normals_view.length()
                        {
                            return Err(GltfError::InvalidView);
                        }

                        let normals_bytes = &normals_bytes
                            [normals_view.offset()..][..normals_view.length()]
                            [normals.offset()..];

                        let mut vertex_count =
                            positions.count().min(normals.count());

                        let vertices = Position3d::from_bytes_iter::<
                            LittleEndian,
                        >(
                            positions_bytes, positions_stride
                        )
                        .zip(Normal3d::from_bytes_iter::<LittleEndian>(
                            normals_bytes,
                            normals_stride,
                        ))
                        .take(vertex_count);

                        align_vec(&mut total_data, 15);
                        let start = total_data.len();
                        for (pos, norm) in vertices {
                            // endian
                            total_data.extend_from_slice(bytemuck::bytes_of(
                                &Position3dNormal3d {
                                    position: pos,
                                    normal: norm,
                                },
                            ));
                        }

                        let posnorms = start..total_data.len();

                        let indices = primitive
                            .indices()
                            .map(|indices| {
                                total_polygons += indices.count();

                                align_vec(
                                    &mut total_data,
                                    15 | (align_of::<u32>() - 1),
                                );
                                Ok::<_, GltfError>((
                                    indices.count(),
                                    load_indices(
                                        &indices,
                                        repr.gltf
                                            .blob
                                            .as_ref()
                                            .map(std::ops::Deref::deref),
                                        &repr.sources,
                                        &mut total_data,
                                    )?,
                                ))
                            })
                            .transpose()?;

                        // let positions = positions.map(|(count, positions)| {
                        //     vertex_count = vertex_count.min(count);
                        //     positions
                        // });

                        // let normals = normals.map(|(count, normals)| {
                        //     vertex_count = vertex_count.min(count);
                        //     normals
                        // });

                        let mut count = vertex_count;
                        let indices = indices.map(|(index_count, indices)| {
                            count = index_count;
                            indices
                        });

                        Ok(MeshAux {
                            // positions,
                            // normals,
                            posnorms,
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
                    .collect()
            })
            .collect::<Result<_, _>>()?;

        let total_meshes: usize = meshes.iter().map(|p| p.len()).sum();
        tracing::debug!("There are {} meshes total", total_meshes);

        let buffer = device.create_buffer_static(
            BufferInfo {
                align: 255,
                size: u64::try_from(total_data.len())
                    .map_err(|_| GltfError::OutOfMemory)?,
                usage: repr.usage,
                memory: MemoryUsageFlags::UPLOAD,
            },
            &total_data,
        )?;

        let mut meshes = meshes
            .into_iter()
            .map(|meshes| {
                meshes
                    .into_iter()
                    .map(|mesh| {
                        let mut bindings = Vec::new();

                        // if let Some(positions) = mesh.positions {
                        //     bindings.push(Binding {
                        //         buffer: buffer.clone(),
                        //         offset: positions.start as u64,
                        //         layout: Position3d::layout(),
                        //     });
                        // }

                        // if let Some(normals) = mesh.normals {
                        //     bindings.push(Binding {
                        //         buffer: buffer.clone(),
                        //         offset: normals.start as u64,
                        //         layout: Normal3d::layout(),
                        //     });
                        // }

                        bindings.push(Binding {
                            buffer: buffer.clone(),
                            offset: mesh.posnorms.start as u64,
                            layout: Position3dNormal3d::layout(),
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
                    .collect()
            })
            .collect();

        let nodes = repr
            .gltf
            .nodes()
            .map(|node| {
                // let transform = match node.transform() {
                //     gltf::scene::Transform::Matrix { matrix: m } => {
                //         Mat4::from([
                //             m[0][0], m[1][0], m[2][0], m[3][0], m[0][1],
                //             m[1][1], m[2][1], m[3][1], m[0][2], m[1][2],
                //             m[2][2], m[3][2], m[0][3], m[1][3], m[2][3],
                //             m[3][3],
                //         ])
                //     }
                //     gltf::scene::Transform::Decomposed {
                //         translation,
                //         rotation,
                //         scale,
                //     } => {
                //         let rotor = Rotor3::new(
                //             rotation[3],
                //             Bivec3 {
                //                 xy: rotation[0],
                //                 xz: rotation[2],
                //                 yz: rotation[1],
                //             },
                //         )
                //         .into_matrix()
                //         .into_homogeneous();

                //         Mat4::from_translation(translation.into())
                //             * rotor
                //             * Mat4::from_nonuniform_scale(Vec4::new(
                //               scale[0], scale[1], scale[2], 1.0,
                //             ))
                //     }
                // };

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
                    mesh: node.mesh().map(|mesh| mesh.index()),
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
        usage |= if self.raster {
            BufferUsage::INDEX | BufferUsage::VERTEX
        } else {
            BufferUsage::empty()
        };
        usage |= if self.blas {
            BufferUsage::RAY_TRACING
                | BufferUsage::SHADER_DEVICE_ADDRESS
                | BufferUsage::STORAGE
        } else {
            BufferUsage::empty()
        };

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

fn load_vertices<V: GltfVertexType>(
    accessor: &gltf::accessor::Accessor<'_>,
    blob: Option<&[u8]>,
    sources: &HashMap<String, Arc<[u8]>>,
    output: &mut Vec<u8>,
) -> Result<Range<usize>, GltfError> {
    if V::DIMENSIONS != accessor.dimensions() {
        return Err(GltfError::Unsupported);
    }

    if V::DATA_TYPE != accessor.data_type() {
        return Err(GltfError::Unsupported);
    }

    if size_of::<V>() != accessor.size() {
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
    let start = output.len();

    // glTF explicitly defines the endianness of binary data as little
    // #[cfg(target_endian = "little")]
    // {
    //     if stride == size_of::<V>() {
    //         // Just copy bytes for packed vertices
    //         // if endianess is the same.
    //         output.extend_from_slice(unsafe {
    //             std::slice::from_raw_parts(
    //                 bytes.as_ptr() as *const _,
    //                 bytes.len(),
    //             )
    //         });
    //         return Ok(start..output.len());
    //     }
    // }

    let vertices = V::from_bytes_iter::<LittleEndian>(bytes, stride)
        .take(accessor.count());

    let mut count = 0;
    for vertex in vertices {
        // endian
        output.extend_from_slice(bytemuck::bytes_of(&vertex));
        count += 1;
    }

    assert_eq!(accessor.count(), count);

    Ok(start..output.len())
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
                output.extend(index.to_ne_bytes().iter().copied());
                count += 1;
            }
            assert_eq!(accessor.count(), count);

            Ok(IndicesAux::U16(start..output.len()))
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

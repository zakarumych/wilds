use {
    super::{align_vec, GltfLoadingError, GltfRepr},
    crate::renderer::{
        Binding, Context, FromBytes, Indices, Joints, Material, MeshBuilder,
        Normal3d, Position3d, PositionNormalTangent3dUV, Renderable, Skin,
        Tangent3d, VertexType, Weights, UV,
    },
    byteorder::{ByteOrder as _, LittleEndian},
    gltf::accessor::{Accessor, DataType, Dimensions},
    illume::*,
    std::{
        convert::{TryFrom as _, TryInto as _},
        marker::PhantomData,
        mem::size_of,
        ops::Range,
    },
};

pub fn load_gltf_primitive(
    repr: &GltfRepr,
    primitive: gltf::Primitive,
    materials: &[Material],
    ctx: &mut Context,
) -> Result<Renderable, GltfLoadingError> {
    let topology = match primitive.mode() {
        gltf::mesh::Mode::Points => PrimitiveTopology::PointList,
        gltf::mesh::Mode::Lines => PrimitiveTopology::LineList,
        gltf::mesh::Mode::LineLoop => {
            return Err(GltfLoadingError::UnsupportedTopology {
                unsupported: gltf::mesh::Mode::LineLoop,
            });
        }
        gltf::mesh::Mode::LineStrip => PrimitiveTopology::LineStrip,
        gltf::mesh::Mode::Triangles => PrimitiveTopology::TriangleList,
        gltf::mesh::Mode::TriangleStrip => PrimitiveTopology::TriangleStrip,
        gltf::mesh::Mode::TriangleFan => PrimitiveTopology::TriangleFan,
    };

    let mut loaded_data = Vec::new();

    let (vectors, skin, vertex_count) =
        load_vertices(repr, primitive.clone(), &mut loaded_data)?;

    let mut count = vertex_count;
    let indices = primitive
        .indices()
        .map(|indices| {
            count = indices.count();

            align_vec(&mut loaded_data, 15);

            load_indices(repr, indices, &mut loaded_data)
        })
        .transpose()?;

    let count = count.try_into().map_err(|_| OutOfMemory)?;
    let vertex_count = vertex_count.try_into().map_err(|_| OutOfMemory)?;

    let buffer = ctx.create_buffer_static(
        BufferInfo {
            align: 255,
            size: u64::try_from(loaded_data.len()).map_err(|_| OutOfMemory)?,
            usage: repr.config.mesh_indices_usage
                | repr.config.mesh_vertices_usage,
        },
        &loaded_data,
    )?;

    let mut bindings = Vec::new();

    bindings.push(Binding {
        buffer: buffer.clone(),
        offset: vectors.start as u64,
        layout: PositionNormalTangent3dUV::layout(),
    });

    if let Some(skin) = skin {
        bindings.push(Binding {
            buffer: buffer.clone(),
            offset: skin.start as u64,
            layout: Skin::layout(),
        });
    }

    let indices = match indices {
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
    };

    let mesh = MeshBuilder {
        bindings,
        indices,
        topology,
    };

    let mesh = mesh.build(count, vertex_count);

    let material = match primitive.material().index() {
        Some(material) => materials[material].clone(),
        None => Material::new(),
    };

    Ok(Renderable { mesh, material })
}

enum IndicesAux {
    U16(Range<usize>),
    U32(Range<usize>),
}

fn load_indices(
    repr: &GltfRepr,
    accessor: Accessor<'_>,
    output: &mut Vec<u8>,
) -> Result<IndicesAux, GltfLoadingError> {
    if Dimensions::Scalar != accessor.dimensions() {
        return Err(GltfLoadingError::UnexpectedDimensions {
            unexpected: accessor.dimensions(),
            expected: &[Dimensions::Scalar],
        });
    }

    let view = accessor
        .view()
        .ok_or(GltfLoadingError::SparseAccessorUnsupported)?;

    let stride = view.stride().unwrap_or(accessor.size());

    let accessor_size = if accessor.count() == 0 {
        0
    } else {
        (accessor.count() - 1) * stride + accessor.size()
    };

    if view.length() < accessor_size + accessor.offset() {
        return Err(GltfLoadingError::AccessorOutOfBound);
    }

    let bytes = match view.buffer().source() {
        gltf::buffer::Source::Bin => repr
            .gltf
            .blob
            .as_deref()
            .ok_or(GltfLoadingError::MissingSource)?,
        gltf::buffer::Source::Uri(uri) => repr
            .buffers
            .get(uri)
            .ok_or(GltfLoadingError::MissingSource)?,
    };

    if bytes.len() < view.offset() + view.length() {
        return Err(GltfLoadingError::ViewOutOfBound);
    }

    let bytes = &bytes[view.offset() + accessor.offset()..][..accessor_size];

    // glTF explicitly defines the endianness of binary data as little endian
    match accessor.data_type() {
        DataType::U16 => {
            assert_eq!(size_of::<u16>(), accessor.size());

            let start = output.len();
            let mut count = 0;
            for index in u16::from_bytes_iter::<LittleEndian>(bytes, stride)
                .take(accessor.count())
            {
                // FIXME: Support 16-bit indices.
                output.extend((index as u32).to_ne_bytes().iter().copied());
                count += 1;
            }
            assert_eq!(accessor.count(), count, "Not enough indices");

            Ok(IndicesAux::U32(start..output.len()))
        }
        DataType::U32 => {
            assert_eq!(size_of::<u32>(), accessor.size());

            let start = output.len();

            if cfg!(target_endian = "little") && stride == size_of::<u32>() {
                // GLTF defines all data to be in little endian.
                // If indices are packed and host is little endian
                // they can be copied.
                output.extend_from_slice(unsafe {
                    std::slice::from_raw_parts(
                        bytes.as_ptr() as *const _,
                        bytes.len(),
                    )
                });
                Ok(IndicesAux::U32(start..output.len()))
            } else {
                let mut count = 0;
                for index in u32::from_bytes_iter::<LittleEndian>(bytes, stride)
                    .take(accessor.count())
                {
                    output.extend(index.to_ne_bytes().iter().copied());
                    count += 1;
                }
                assert_eq!(accessor.count(), count, "Not enough indices");

                Ok(IndicesAux::U32(start..output.len()))
            }
        }
        unexpected => Err(GltfLoadingError::UnexpectedDataType {
            unexpected,
            expected: &[DataType::U16, DataType::U32],
        }),
    }
}

trait GltfVertexType: VertexType {
    const DIMENSIONS: Dimensions;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self>;

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError>;
}

struct FromGltfBytesIter<'a, T> {
    bytes: &'a [u8],
    stride: usize,
    data_type: DataType,
    marker: PhantomData<fn() -> T>,
}

impl<'a, T> Iterator for FromGltfBytesIter<'a, T>
where
    T: GltfVertexType,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.bytes.len() >= self.stride {
            let v = GltfVertexType::from_bytes(
                self.data_type,
                &self.bytes[..self.stride],
            )?;
            self.bytes = &self.bytes[self.stride..];
            Some(v)
        } else {
            self.bytes = &[];
            None
        }
    }
}

impl GltfVertexType for Position3d {
    const DIMENSIONS: Dimensions = Dimensions::Vec3;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        debug_assert_eq!(data_type, DataType::F32, "Wrong data type");

        if bytes.len() >= size_of::<Self>() {
            Some(FromBytes::from_bytes::<LittleEndian>(
                &bytes[..size_of::<Self>()],
            ))
        } else {
            None
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        if data_type != DataType::F32 {
            Err(GltfLoadingError::UnexpectedDataType {
                expected: &[DataType::F32],
                unexpected: data_type,
            })
        } else {
            Ok(FromGltfBytesIter {
                bytes,
                stride,
                data_type,
                marker: PhantomData,
            })
        }
    }
}

impl GltfVertexType for Normal3d {
    const DIMENSIONS: Dimensions = Dimensions::Vec3;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        debug_assert_eq!(data_type, DataType::F32);

        if bytes.len() >= size_of::<Self>() {
            Some(FromBytes::from_bytes::<LittleEndian>(
                &bytes[..size_of::<Self>()],
            ))
        } else {
            None
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        if data_type != DataType::F32 {
            Err(GltfLoadingError::UnexpectedDataType {
                unexpected: data_type,
                expected: &[DataType::F32],
            })
        } else {
            Ok(FromGltfBytesIter {
                bytes,
                stride,
                data_type,
                marker: PhantomData,
            })
        }
    }
}

impl GltfVertexType for Tangent3d {
    const DIMENSIONS: Dimensions = Dimensions::Vec4;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        debug_assert_eq!(data_type, DataType::F32);

        if bytes.len() >= size_of::<Self>() {
            Some(FromBytes::from_bytes::<LittleEndian>(
                &bytes[..size_of::<Self>()],
            ))
        } else {
            None
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        if data_type != DataType::F32 {
            Err(GltfLoadingError::UnexpectedDataType {
                expected: &[DataType::F32],
                unexpected: data_type,
            })
        } else {
            Ok(FromGltfBytesIter {
                bytes,
                stride,
                data_type,
                marker: PhantomData,
            })
        }
    }
}

impl GltfVertexType for UV {
    const DIMENSIONS: Dimensions = Dimensions::Vec2;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        match data_type {
            DataType::U8 => {
                if let [u, v, ..] = *bytes {
                    Some(UV([u as f32 / 255.0, v as f32 / 255.0]))
                } else {
                    None
                }
            }
            DataType::U16 => {
                let size = size_of::<[u16; 2]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut uv = [0; 2];
                    LittleEndian::read_u16_into(&bytes[..size], &mut uv);
                    let [u, v] = uv;
                    Some(UV([u as f32 / 255.0, v as f32 / 255.0]))
                }
            }
            DataType::F32 => {
                let size = size_of::<[f32; 2]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut uv = [0.0; 2];
                    LittleEndian::read_f32_into(&bytes[..size], &mut uv);
                    Some(UV(uv))
                }
            }
            _ => unreachable!(),
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        match data_type {
            DataType::U8 | DataType::U16 | DataType::F32 => {
                Ok(FromGltfBytesIter {
                    bytes,
                    stride,
                    data_type,
                    marker: PhantomData,
                })
            }
            _ => Err(GltfLoadingError::UnexpectedDataType {
                unexpected: data_type,
                expected: &[DataType::U8, DataType::U16, DataType::F32],
            }),
        }
    }
}

impl GltfVertexType for Joints {
    const DIMENSIONS: Dimensions = Dimensions::Vec4;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        match data_type {
            DataType::U8 => {
                if let [a, b, c, d] = *bytes {
                    Some(Joints([a as u32, b as u32, c as u32, d as u32]))
                } else {
                    None
                }
            }
            DataType::U16 => {
                let size = size_of::<[u16; 4]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut joints = [0; 4];
                    LittleEndian::read_u16_into(&bytes[..size], &mut joints);
                    let [a, b, c, d] = joints;
                    Some(Joints([a as u32, b as u32, c as u32, d as u32]))
                }
            }
            DataType::U32 => {
                let size = size_of::<[u32; 4]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut joints = [0; 4];
                    LittleEndian::read_u32_into(&bytes[..size], &mut joints);
                    Some(Joints(joints))
                }
            }
            _ => unreachable!(),
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        match data_type {
            DataType::U8 | DataType::U16 => Ok(FromGltfBytesIter {
                bytes,
                stride,
                data_type,
                marker: PhantomData,
            }),
            _ => Err(GltfLoadingError::UnexpectedDataType {
                unexpected: data_type,
                expected: &[DataType::U8, DataType::U16],
            }),
        }
    }
}

impl GltfVertexType for Weights {
    const DIMENSIONS: Dimensions = Dimensions::Vec4;

    fn from_bytes(data_type: DataType, bytes: &[u8]) -> Option<Self> {
        match data_type {
            DataType::U8 => {
                if let [a, b, c, d] = *bytes {
                    Some(Weights([
                        a as f32 / 255.0,
                        b as f32 / 255.0,
                        c as f32 / 255.0,
                        d as f32 / 255.0,
                    ]))
                } else {
                    None
                }
            }
            DataType::U16 => {
                let size = size_of::<[u16; 4]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut weights = [0; 4];
                    LittleEndian::read_u16_into(&bytes[..size], &mut weights);
                    let [a, b, c, d] = weights;
                    Some(Weights([
                        a as f32 / 65535.0,
                        b as f32 / 65535.0,
                        c as f32 / 65535.0,
                        d as f32 / 65535.0,
                    ]))
                }
            }
            DataType::F32 => {
                let size = size_of::<[f32; 4]>();
                if bytes.len() < size {
                    None
                } else {
                    let mut weights = [0.0; 4];
                    LittleEndian::read_f32_into(&bytes[..size], &mut weights);
                    Some(Weights(weights))
                }
            }
            _ => unreachable!(),
        }
    }

    fn from_bytes_iter<'a>(
        data_type: DataType,
        bytes: &'a [u8],
        stride: usize,
    ) -> Result<FromGltfBytesIter<'a, Self>, GltfLoadingError> {
        match data_type {
            DataType::U8 | DataType::U16 | DataType::F32 => {
                Ok(FromGltfBytesIter {
                    bytes,
                    stride,
                    data_type,
                    marker: PhantomData,
                })
            }
            _ => Err(GltfLoadingError::UnexpectedDataType {
                unexpected: data_type,
                expected: &[DataType::U8, DataType::U16, DataType::F32],
            }),
        }
    }
}

fn load_vertex_attribute<'a, V: GltfVertexType>(
    repr: &'a GltfRepr,
    accessor: Accessor<'_>,
) -> Result<impl Iterator<Item = V> + 'a, GltfLoadingError> {
    if V::DIMENSIONS != accessor.dimensions() {
        return Err(GltfLoadingError::UnexpectedDimensions {
            unexpected: accessor.dimensions(),
            expected: &[V::DIMENSIONS],
        });
    }

    let view = accessor
        .view()
        .ok_or(GltfLoadingError::SparseAccessorUnsupported)?;

    let stride = view.stride().unwrap_or(accessor.size());

    let accessor_size = if accessor.count() == 0 {
        0
    } else {
        (accessor.count() - 1) * stride + accessor.size()
    };

    if view.length() < accessor_size + accessor.offset() {
        tracing::error!(
            "Accessor to vertex attribute '{}' is out of its buffer view bounds",
            V::NAME
        );
        return Err(GltfLoadingError::AccessorOutOfBound);
    }

    let bytes = match view.buffer().source() {
        gltf::buffer::Source::Bin => repr.gltf.blob.as_deref().ok_or(GltfLoadingError::MissingSource)?,
        gltf::buffer::Source::Uri(uri) => {
            repr.buffers.get(uri).ok_or_else(|| {
                tracing::error!(
                    "View of accessor to vertex attribute '{}' has non-existent source {}",
                    V::NAME, uri
                );
                GltfLoadingError::MissingSource
            })?
        }
    };

    if bytes.len() < view.offset() + view.length() {
        tracing::error!(
            "View of accessor to vertex attribute '{}' is out of its buffer bounds",
            V::NAME
        );
        return Err(GltfLoadingError::ViewOutOfBound);
    }

    let bytes = &bytes[view.offset() + accessor.offset()..][..accessor_size];

    // glTF explicitly defines that binary data is in little-endian.
    GltfVertexType::from_bytes_iter(accessor.data_type(), bytes, stride)
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

fn load_vertices(
    repr: &GltfRepr,
    primitive: gltf::mesh::Primitive<'_>,
    output: &mut Vec<u8>,
) -> Result<(Range<usize>, Option<Range<usize>>, usize), GltfLoadingError> {
    let position = primitive
        .get(&gltf::Semantic::Positions)
        .ok_or(GltfLoadingError::MissingPositionAttribute)?;

    let position_attribute_iter =
        load_vertex_attribute::<Position3d>(repr, position)?;

    let normals_attribute_iter = primitive
        .get(&gltf::Semantic::Normals)
        .map(|normals| load_vertex_attribute::<Normal3d>(repr, normals))
        .transpose()?;

    let normals_attribute_iter =
        iter_or_defaults(normals_attribute_iter, Normal3d([0.0; 3]));

    let tangents_attribute_iter = primitive
        .get(&gltf::Semantic::Tangents)
        .map(|tangents| load_vertex_attribute::<Tangent3d>(repr, tangents))
        .transpose()?;

    let tangents_attribute_iter =
        iter_or_defaults(tangents_attribute_iter, Tangent3d([0.0; 4]));

    let uv_attribute_iter = primitive
        .get(&gltf::Semantic::TexCoords(0))
        .map(|uv| load_vertex_attribute::<UV>(repr, uv))
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

    let vectors = start..output.len();

    if let (Some(joints), Some(weights)) = (
        primitive.get(&gltf::Semantic::Joints(0)),
        primitive.get(&gltf::Semantic::Weights(0)),
    ) {
        let joints = load_vertex_attribute::<Joints>(repr, joints)?;
        let weights = load_vertex_attribute::<Weights>(repr, weights)?;

        let skin_count = joints
            .zip(weights)
            .map(|(joints, weights)| {
                let skin = Skin { joints, weights };
                output.extend_from_slice(bytemuck::bytes_of(&skin));
            })
            .take(count)
            .count();

        if skin_count < count {
            tracing::error!("Too few joints and weights in skinned mesh");
            for _ in skin_count..count {
                let skin = Skin {
                    joints: Joints([0; 4]),
                    weights: Weights([0.0; 4]),
                };
                output.extend_from_slice(bytemuck::bytes_of(&skin));
            }
        }

        let skin = vectors.end..output.len();

        Ok((vectors, Some(skin), count))
    } else {
        Ok((vectors, None, count))
    }
}

use super::{vertex::VertexLayout, Context};
use bumpalo::{collections::Vec as BVec, Bump};
use illume::*;
use std::{
    borrow::Cow, convert::TryFrom as _, mem::size_of_val, ops::Range, sync::Arc,
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Binding {
    pub buffer: Buffer,
    pub offset: u64,
    pub layout: VertexLayout,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Indices {
    pub buffer: Buffer,
    pub offset: u64,
    pub index_type: IndexType,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct MeshBuilder {
    pub bindings: Vec<Binding>,
    pub indices: Option<Indices>,
    pub topology: PrimitiveTopology,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self::with_topology(PrimitiveTopology::TriangleList)
    }

    pub fn with_topology(topology: PrimitiveTopology) -> Self {
        MeshBuilder {
            bindings: Vec::new(),
            indices: None,
            topology,
        }
    }

    pub fn with_binding(
        mut self,
        buffer: Buffer,
        offset: u64,
        layout: VertexLayout,
    ) -> Self {
        self.add_binding(buffer, offset, layout);

        self
    }

    pub fn add_binding(
        &mut self,
        buffer: Buffer,
        offset: u64,
        layout: VertexLayout,
    ) -> &mut Self {
        self.bindings.push(Binding {
            buffer,
            offset,
            layout,
        });

        self
    }

    pub fn with_indices(
        mut self,
        buffer: Buffer,
        offset: u64,
        index_type: IndexType,
    ) -> Self {
        self.set_indices(buffer, offset, index_type);

        self
    }

    pub fn set_indices(
        &mut self,
        buffer: Buffer,
        offset: u64,
        index_type: IndexType,
    ) -> &mut Self {
        self.indices = Some(Indices {
            buffer,
            offset,
            index_type,
        });

        self
    }

    pub fn build(self, count: u32, vertex_count: u32) -> Mesh {
        Mesh {
            bindings: self.bindings.into(),
            indices: self.indices,
            topology: self.topology,
            count,
            vertex_count,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Mesh {
    bindings: Arc<[Binding]>,
    indices: Option<Indices>,
    count: u32,
    vertex_count: u32,
    topology: PrimitiveTopology,
}

impl Mesh {
    pub fn builder() -> MeshBuilder {
        MeshBuilder::new()
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn bindings(&self) -> &[Binding] {
        &*self.bindings
    }

    pub fn indices(&self) -> Option<&Indices> {
        self.indices.as_ref()
    }

    pub fn build_triangles_blas<'a>(
        &self,
        encoder: &mut Encoder<'a>,
        device: &Device,
        bump: &'a Bump,
    ) -> Result<AccelerationStructure, OutOfMemory> {
        assert_eq!(self.topology, PrimitiveTopology::TriangleList);

        assert_eq!(self.count % 3, 0);

        let triangle_count = self.count / 3;

        let pos_binding: &Binding = &self.bindings[0];

        let pos_layout = &pos_binding.layout;

        assert_eq!(pos_layout.rate, VertexInputRate::Vertex);

        let pos_location = pos_layout.locations.as_ref()[0];

        let pos_address = device
            .get_buffer_device_address(&self.bindings[0].buffer)
            .unwrap()
            .offset(pos_binding.offset)
            .offset(pos_location.offset.into());

        let blas = device.create_acceleration_structure(
            AccelerationStructureInfo {
                level: AccelerationStructureLevel::Bottom,
                flags: AccelerationStructureFlags::PREFER_FAST_TRACE,
                geometries: vec![
                    AccelerationStructureGeometryInfo::Triangles {
                        max_primitive_count: triangle_count,
                        index_type: self
                            .indices
                            .as_ref()
                            .map(|indices| indices.index_type),
                        max_vertex_count: self.count,
                        vertex_format: pos_location.format,
                        allows_transforms: true,
                    },
                ],
            },
        )?;

        let blas_scratch = device
            .allocate_acceleration_structure_build_scratch(&blas, false)?;

        let blas_scratch_address =
            device.get_buffer_device_address(&blas_scratch).unwrap();

        let geometries =
            bump.alloc([AccelerationStructureGeometry::Triangles {
                flags: GeometryFlags::empty(),
                vertex_format: Format::RGB32Sfloat,
                vertex_data: pos_address,
                vertex_stride: pos_layout.stride.into(),
                first_vertex: 0,
                primitive_count: triangle_count,
                index_data: self.indices.as_ref().map(|indices| {
                    let index_address = device
                        .get_buffer_device_address(&indices.buffer)
                        .unwrap()
                        .offset(indices.offset);

                    match indices.index_type {
                        IndexType::U16 => IndexData::U16(index_address),
                        IndexType::U32 => IndexData::U32(index_address),
                    }
                }),
                transform_data: None,
            }]);

        let infos = bump.alloc([AccelerationStructureBuildGeometryInfo {
            src: None,
            dst: blas.clone(),
            geometries,
            scratch: blas_scratch_address,
        }]);

        encoder.build_acceleration_structure(infos);

        Ok(blas)
    }

    pub fn draw<'a>(
        &self,
        instances: Range<u32>,
        layouts: &[VertexLayout],
        encoder: &mut RenderPassEncoder<'_, 'a>,
        bump: &'a Bump,
    ) -> bool {
        let mut to_bind = BVec::with_capacity_in(self.bindings.len(), bump);

        for layout in layouts {
            for binding in &*self.bindings {
                if binding.layout == *layout {
                    to_bind.push((binding.buffer.clone(), binding.offset));

                    break;
                }
            }

            tracing::trace!(
                "Cannot find vertex bindings for all requestd vertex layouts"
            );

            return false;
        }

        encoder.bind_vertex_buffers(0, to_bind.into_bump_slice());

        if let Some(indices) = &self.indices {
            encoder.bind_index_buffer(
                bump.alloc(indices.buffer.clone()),
                indices.offset,
                indices.index_type,
            );

            encoder.draw_indexed(0..self.count, 0, instances);
        } else {
            encoder.draw(0..self.count, instances);
        }

        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct BindingData<'a> {
    #[cfg_attr(
        feature = "serde-1",
        serde(with = "serde_bytes", borrow = "'a")
    )]
    pub data: Cow<'a, [u8]>,
    pub layout: VertexLayout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct IndicesData<'a> {
    #[cfg_attr(
        feature = "serde-1",
        serde(with = "serde_bytes", borrow = "'a")
    )]
    pub data: Cow<'a, [u8]>,
    pub index_type: IndexType,
}

impl<'a> From<&'a [u16]> for IndicesData<'a> {
    fn from(indices: &'a [u16]) -> Self {
        IndicesData {
            data: unsafe {
                std::slice::from_raw_parts(
                    indices.as_ptr() as *const u8,
                    size_of_val(indices),
                )
            }
            .into(),
            index_type: IndexType::U16,
        }
    }
}

impl<'a> From<&'a [u32]> for IndicesData<'a> {
    fn from(indices: &'a [u32]) -> Self {
        IndicesData {
            data: unsafe {
                std::slice::from_raw_parts(
                    indices.as_ptr() as *const u8,
                    size_of_val(indices),
                )
            }
            .into(),
            index_type: IndexType::U32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshData<'a> {
    #[cfg_attr(
        feature = "serde-1",
        serde(skip_serializing_if = "Vec::is_empty", default, borrow = "'a")
    )]
    pub bindings: Vec<BindingData<'a>>,
    #[cfg_attr(
        feature = "serde-1",
        serde(skip_serializing_if = "Option::is_none", default, borrow = "'a")
    )]
    pub indices: Option<IndicesData<'a>>,
    #[cfg_attr(
        feature = "serde-1",
        serde(
            skip_serializing_if = "topology_is_triangles",
            default = "topology_triangles"
        )
    )]
    pub topology: PrimitiveTopology,
}

impl MeshData<'_> {
    pub fn build(
        &self,
        ctx: &mut Context,
        vertices_usage: BufferUsage,
        indices_usage: BufferUsage,
    ) -> Result<Mesh, OutOfMemory> {
        let mut min_vertex_count = !0u32;

        let bindings: Arc<[Binding]> = self
            .bindings
            .iter()
            .map(|binding| -> Result<_, OutOfMemory> {
                let vertex_count = u64::try_from(binding.data.len())
                    .map_err(|_| OutOfMemory)?
                    / u64::from(binding.layout.stride);

                let vertex_count =
                    u32::try_from(vertex_count).map_err(|_| OutOfMemory)?;

                min_vertex_count = min_vertex_count.min(vertex_count);

                Ok(Binding {
                    buffer: ctx.create_buffer_static(
                        BufferInfo {
                            align: 255,
                            size: u64::try_from(binding.data.len())
                                .map_err(|_| OutOfMemory)?,
                            usage: vertices_usage,
                            memory: MemoryUsageFlags::empty(),
                        },
                        &binding.data,
                    )?,
                    offset: 0,
                    layout: binding.layout.clone(),
                })
            })
            .collect::<Result<_, _>>()?;

        let mut count = min_vertex_count;

        let indices = self
            .indices
            .as_ref()
            .map(|indices| -> Result<_, OutOfMemory> {
                let index_count = u64::try_from(indices.data.len())
                    .map_err(|_| OutOfMemory)?
                    / u64::from(indices.index_type.size());

                count = u32::try_from(index_count).map_err(|_| OutOfMemory)?;

                Ok(Indices {
                    buffer: ctx.create_buffer_static(
                        BufferInfo {
                            align: 255,
                            size: u64::try_from(indices.data.len())
                                .map_err(|_| OutOfMemory)?,
                            usage: indices_usage,
                            memory: MemoryUsageFlags::empty(),
                        },
                        &indices.data,
                    )?,
                    offset: 0,
                    index_type: indices.index_type,
                })
            })
            .transpose()?;

        Ok(Mesh {
            bindings,
            indices,
            topology: self.topology,
            count,
            vertex_count: min_vertex_count,
        })
    }

    pub fn build_for_raster(
        &self,
        ctx: &mut Context,
    ) -> Result<Mesh, OutOfMemory> {
        self.build(ctx, BufferUsage::VERTEX, BufferUsage::INDEX)
    }

    pub fn build_for_blas(
        &self,
        ctx: &mut Context,
    ) -> Result<Mesh, OutOfMemory> {
        self.build(
            ctx,
            BufferUsage::RAY_TRACING | BufferUsage::STORAGE,
            BufferUsage::RAY_TRACING | BufferUsage::STORAGE,
        )
    }

    pub fn build_for_dynamic_blas(
        &self,
        ctx: &mut Context,
    ) -> Result<Mesh, OutOfMemory> {
        self.build(
            ctx,
            BufferUsage::RAY_TRACING | BufferUsage::STORAGE,
            BufferUsage::RAY_TRACING | BufferUsage::STORAGE,
        )
    }
}

fn topology_is_triangles(topology: &PrimitiveTopology) -> bool {
    *topology == PrimitiveTopology::TriangleList
}

fn topology_triangles() -> PrimitiveTopology {
    PrimitiveTopology::TriangleList
}

#[cfg(feature = "genmesh")]
mod gm {
    use super::*;
    use crate::vertex::{
        Color, Normal3d, Position3d, PositionNormal3d, PositionNormal3dColor,
        VertexType,
    };
    use genmesh::{
        generators::{IndexedPolygon, SharedVertex},
        EmitTriangles, Quad, Vertex,
    };
    use std::{convert::TryFrom as _, mem::size_of};

    impl Mesh {
        pub fn from_generator_pos<G>(
            generator: &G,
            usage: BufferUsage,
            device: &Device,
            index_type: IndexType,
        ) -> Result<Self, OutOfMemory>
        where
            G: SharedVertex<Vertex> + IndexedPolygon<Quad<usize>>,
        {
            Self::from_generator(
                generator,
                usage,
                device,
                index_type,
                Position3d::from,
            )
        }

        pub fn from_generator_pos_norm<G>(
            generator: &G,
            usage: BufferUsage,
            device: &Device,
            index_type: IndexType,
        ) -> Result<Self, OutOfMemory>
        where
            G: SharedVertex<Vertex> + IndexedPolygon<Quad<usize>>,
        {
            Self::from_generator(
                generator,
                usage,
                device,
                index_type,
                PositionNormal3d::from,
            )
        }

        pub fn from_generator_pos_norm_fixed_color<G>(
            generator: &G,
            usage: BufferUsage,
            device: &Device,
            index_type: IndexType,
            color: Color,
        ) -> Result<Self, OutOfMemory>
        where
            G: SharedVertex<Vertex> + IndexedPolygon<Quad<usize>>,
        {
            Self::from_generator(generator, usage, device, index_type, |v| {
                PositionNormal3dColor {
                    position: v.into(),
                    normal: v.into(),
                    color,
                }
            })
        }

        pub fn from_generator<G, V, P>(
            generator: &G,
            usage: BufferUsage,
            ctx: &mut Context,
            index_type: IndexType,
            vertex: impl Fn(Vertex) -> V,
        ) -> Result<Self, OutOfMemory>
        where
            G: SharedVertex<Vertex> + IndexedPolygon<P>,
            V: VertexType,
            P: EmitTriangles<Vertex = usize>,
        {
            assert_eq!(
                size_of::<V>(),
                usize::try_from(V::layout().stride).unwrap()
            );

            let vertices: Vec<_> =
                generator.shared_vertex_iter().map(vertex).collect();

            let vertices_size = size_of_val(&vertices[..]);

            let indices_offset = ((vertices_size - 1) | 15) + 1;

            let mut data;

            let vertex_count =
                u32::try_from(vertices.len()).map_err(|_| OutOfMemory)?;

            let index_count;

            let align_data_len = |data_len: usize| ((data_len - 1) | 15) + 1;

            match index_type {
                IndexType::U16 => {
                    let indices: Vec<_> = generator
                        .indexed_polygon_iter()
                        .flat_map(|polygon| {
                            let mut indices = Vec::new();

                            polygon.emit_triangles(|triangle| {
                                indices.push(triangle.x);

                                indices.push(triangle.y);

                                indices.push(triangle.z);
                            });

                            indices
                        })
                        .map(|index| u16::try_from(index).unwrap())
                        .collect();

                    index_count = u32::try_from(indices.len())
                        .map_err(|_| OutOfMemory)?;

                    let indices_size = size_of_val(&indices[..]);

                    data = vec![
                        0u8;
                        align_data_len(indices_offset + indices_size)
                    ];

                    unsafe {
                        data[..vertices_size].copy_from_slice(
                            std::slice::from_raw_parts(
                                &vertices[0] as *const _ as *const _,
                                vertices_size,
                            ),
                        );

                        data[indices_offset..indices_offset + indices_size]
                            .copy_from_slice(std::slice::from_raw_parts(
                                &indices[0] as *const _ as *const _,
                                indices_size,
                            ));
                    }
                }

                IndexType::U32 => {
                    let indices: Vec<_> = generator
                        .indexed_polygon_iter()
                        .flat_map(|polygon| {
                            let mut indices = Vec::new();

                            polygon.emit_triangles(|triangle| {
                                indices.push(triangle.x);

                                indices.push(triangle.y);

                                indices.push(triangle.z);
                            });

                            indices
                        })
                        .map(|index| u32::try_from(index).unwrap())
                        .collect();

                    index_count = u32::try_from(indices.len())
                        .map_err(|_| OutOfMemory)?;

                    let indices_size = size_of_val(&indices[..]);

                    data = vec![
                        0u8;
                        align_data_len(indices_offset + indices_size)
                    ];

                    unsafe {
                        data[..vertices_size].copy_from_slice(
                            std::slice::from_raw_parts(
                                &vertices[0] as *const _ as *const _,
                                vertices_size,
                            ),
                        );

                        data[indices_offset..indices_offset + indices_size]
                            .copy_from_slice(std::slice::from_raw_parts(
                                &indices[0] as *const _ as *const _,
                                indices_size,
                            ));
                    }
                }
            }

            let buffer = ctx.create_buffer_static(
                BufferInfo {
                    align: 63,
                    size: u64::try_from(data.len()).map_err(|_| OutOfMemory)?,
                    usage,
                    memory: MemoryUsageFlags::empty(),
                },
                &data[..],
            )?;

            let binding = Binding {
                buffer: buffer.clone(),
                offset: 0,
                layout: V::layout(),
            };

            let indices = Indices {
                buffer: buffer.clone(),
                offset: u64::try_from(indices_offset).unwrap(),
                index_type,
            };

            Ok(Mesh {
                bindings: Arc::new([binding]),
                indices: Some(indices),
                count: index_count,
                topology: PrimitiveTopology::TriangleList,
                vertex_count,
            })
        }
    }
}

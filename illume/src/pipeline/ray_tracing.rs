use crate::PipelineLayout;
use crate::{
    buffer::{Buffer, StridedBufferRegion},
    format::Format,
    resource::{Handle, ResourceTrait},
    shader::Shader,
    DeviceAddress, IndexType,
};
use std::ops::Range;

define_handle! {
    /// Bottom-level acceleration structure.
    pub struct AccelerationStructure(AccelerationStructureInfo);
}

bitflags::bitflags! {
    /// Bits which can be set in `AccelerationStructureInfo` specifying additional parameters for acceleration structure builds.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct AccelerationStructureFlags: u32 {
        const ALLOW_UPDATE      = 0x00000001;
        const ALLOW_COMPACTION  = 0x00000002;
        const PREFER_FAST_TRACE = 0x00000004;
        const PREFER_FAST_BUILD = 0x00000008;
        const LOW_MEMORY        = 0x00000010;
    }
}

bitflags::bitflags! {
    /// Bits specifying additional parameters for geometries in acceleration structure builds
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct GeometryFlags: u32 {
        const OPAQUE                            = 0x00000001;
        const NO_DUPLICATE_ANY_HIT_INVOCATION   = 0x00000002;
    }
}

bitflags::bitflags! {
    /// Possible values of flags in the instance modifying the behavior of that instance.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct GeometryInstanceFlags: u8 {
        const TRIANGLE_FACING_CULL_DISABLE    = 0x00000001;
        const TRIANGLE_FRONT_COUNTERCLOCKWISE = 0x00000002;
        const FORCE_OPAQUE                    = 0x00000004;
        const FORCE_NO_OPAQUE                 = 0x00000008;
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct AccelerationStructureInfo {
    pub level: AccelerationStructureLevel,
    pub flags: AccelerationStructureFlags,
    pub geometries: Vec<AccelerationStructureGeometryInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum AccelerationStructureLevel {
    Bottom,
    Top,
}

/// Specifies the shape of geometries that will be built into an acceleration
/// structure.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum AccelerationStructureGeometryInfo {
    Triangles {
        /// Maximum number of primitives that can be built into an acceleration
        /// structure geometry.
        max_primitive_count: u32,
        index_type: Option<IndexType>,
        max_vertex_count: u32,
        vertex_format: Format,
        allows_transforms: bool,
    },
    AABBs {
        /// Maximum number of primitives that can be built into an acceleration
        /// structure geometry.
        max_primitive_count: u32,
    },
    Instances {
        /// Maximum number of primitives that can be built into an acceleration
        /// structure geometry.
        max_primitive_count: u32,
    },
}

impl AccelerationStructureGeometryInfo {
    pub fn is_triangles(&self) -> bool {
        match self {
            Self::Triangles { .. } => true,
            _ => false,
        }
    }

    pub fn is_aabbs(&self) -> bool {
        match self {
            Self::AABBs { .. } => true,
            _ => false,
        }
    }

    pub fn is_instances(&self) -> bool {
        match self {
            Self::Instances { .. } => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccelerationStructureBuildGeometryInfo<'a> {
    pub src: Option<AccelerationStructure>,
    pub dst: AccelerationStructure,
    pub geometries: &'a [AccelerationStructureGeometry],
    pub scratch: DeviceAddress,
}

#[derive(Clone, Copy, Debug)]
pub enum AccelerationStructureGeometry {
    Triangles {
        flags: GeometryFlags,
        vertex_format: Format,
        vertex_data: DeviceAddress,
        vertex_stride: u64,
        first_vertex: u32,
        primitive_count: u32,
        index_data: Option<IndexData>,
        transform_data: Option<DeviceAddress>,
    },
    AABBs {
        flags: GeometryFlags,
        data: DeviceAddress,
        stride: u64,
        primitive_count: u32,
    },
    Instances {
        flags: GeometryFlags,
        data: DeviceAddress,
        primitive_count: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum IndexData {
    U16(DeviceAddress),
    U32(DeviceAddress),
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct TransformMatrix {
    pub matrix: [[f32; 4]; 3],
}

impl TransformMatrix {
    pub fn identity() -> Self {
        TransformMatrix {
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        }
    }
}

impl Default for TransformMatrix {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(feature = "ultraviolet")]
impl From<ultraviolet::Mat4> for TransformMatrix {
    fn from(m: ultraviolet::Mat4) -> Self {
        TransformMatrix {
            matrix: [
                [m[0][0], m[1][0], m[2][0], m[3][0]],
                [m[0][1], m[1][1], m[2][1], m[3][1]],
                [m[0][2], m[1][2], m[2][2], m[3][2]],
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(align(8))]
#[repr(C)]
pub struct AabbPositions {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct InstanceCustomIndexAndMask(pub u32);

impl InstanceCustomIndexAndMask {
    pub fn new(custom_index: u32, mask: u8) -> Self {
        assert!(custom_index < 1u32 << 24);

        InstanceCustomIndexAndMask(custom_index | ((mask as u32) << 24))
    }
}

impl From<(u32, u8)> for InstanceCustomIndexAndMask {
    fn from((index, mask): (u32, u8)) -> Self {
        InstanceCustomIndexAndMask::new(index, mask)
    }
}

impl From<u32> for InstanceCustomIndexAndMask {
    fn from(index: u32) -> InstanceCustomIndexAndMask {
        InstanceCustomIndexAndMask::new(index, !0)
    }
}

impl Default for InstanceCustomIndexAndMask {
    fn default() -> Self {
        InstanceCustomIndexAndMask::new(0, !0)
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct InstanceShaderBindingOffsetAndFlags(pub u32);

impl InstanceShaderBindingOffsetAndFlags {
    pub fn new(
        instance_shader_binding_offset: u32,
        flags: GeometryInstanceFlags,
    ) -> Self {
        assert!(instance_shader_binding_offset < 1u32 << 24);

        InstanceShaderBindingOffsetAndFlags(
            instance_shader_binding_offset | ((flags.bits() as u32) << 24),
        )
    }
}

impl From<u32> for InstanceShaderBindingOffsetAndFlags {
    fn from(offset: u32) -> InstanceShaderBindingOffsetAndFlags {
        InstanceShaderBindingOffsetAndFlags::new(
            offset,
            GeometryInstanceFlags::empty(),
        )
    }
}

impl From<(u32, GeometryInstanceFlags)>
    for InstanceShaderBindingOffsetAndFlags
{
    fn from((offset, flags): (u32, GeometryInstanceFlags)) -> Self {
        InstanceShaderBindingOffsetAndFlags::new(offset, flags)
    }
}

impl Default for InstanceShaderBindingOffsetAndFlags {
    fn default() -> Self {
        InstanceShaderBindingOffsetAndFlags::new(
            0,
            GeometryInstanceFlags::empty(),
        )
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(align(16))]
#[repr(C)]
pub struct AccelerationStructureInstance {
    pub transform: TransformMatrix,
    pub custom_index_mask: InstanceCustomIndexAndMask,
    pub shader_binding_offset_flags: InstanceShaderBindingOffsetAndFlags,
    pub acceleration_structure_reference: DeviceAddress,
}

unsafe impl bytemuck::Zeroable for AccelerationStructureInstance {}
unsafe impl bytemuck::Pod for AccelerationStructureInstance {}

impl AccelerationStructureInstance {
    pub fn new(blas_address: DeviceAddress) -> Self {
        AccelerationStructureInstance {
            transform: Default::default(),
            custom_index_mask: Default::default(),
            shader_binding_offset_flags: Default::default(),
            acceleration_structure_reference: blas_address,
        }
    }

    pub fn with_transform(mut self, transform: TransformMatrix) -> Self {
        self.transform = transform;

        self
    }

    pub fn set_transform(&mut self, transform: TransformMatrix) -> &mut Self {
        self.transform = transform;

        self
    }
}

/// Resource that describes whole ray-tracing pipeline state.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct RayTracingPipeline {
    handle: Handle<Self>,
}

impl ResourceTrait for RayTracingPipeline {
    type Info = RayTracingPipelineInfo;

    fn from_handle(handle: Handle<Self>) -> Self {
        Self { handle }
    }

    fn handle(&self) -> &Handle<Self> {
        &self.handle
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RayTracingPipelineInfo {
    /// Array of shaders referenced by indices in shader groups below.
    pub shaders: Vec<Shader>,

    /// Pipline-creation-time layer of indirection between individual shaders
    /// and acceleration structures.
    pub groups: Vec<RayTracingShaderGroupInfo>,

    /// Maximum recursion depth to trace rays.
    pub max_recursion_depth: u32,

    /// Pipeline layout.
    pub layout: PipelineLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum RayTracingShaderGroupInfo {
    Raygen {
        /// Index of raygen shader in `RayTracingPipelineInfo::shaders`.
        raygen: u32,
    },
    Miss {
        /// Index of miss shader in `RayTracingPipelineInfo::shaders`.
        miss: u32,
    },
    Triangles {
        /// Index of any-hit shader in `RayTracingPipelineInfo::shaders`.
        any_hit: Option<u32>,
        /// Index of closest-hit shader in `RayTracingPipelineInfo::shaders`.
        closest_hit: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderBindingTableInfo<'a> {
    pub raygen: Option<u32>,
    pub miss: &'a [u32],
    pub hit: &'a [u32],
    pub callable: &'a [u32],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderBindingTable {
    pub raygen: Option<StridedBufferRegion>,
    pub miss: Option<StridedBufferRegion>,
    pub hit: Option<StridedBufferRegion>,
    pub callable: Option<StridedBufferRegion>,
}

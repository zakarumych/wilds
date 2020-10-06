use {
    super::{Pass, SparseDescriptors},
    crate::renderer::{Context, Mesh, Texture},
    bumpalo::Bump,
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    hecs::World,
    illume::*,
    nalgebra as na,
    std::collections::HashMap,
};

pub struct Input<'a> {
    pub grid_factor: f32,
    pub extent: Extent3d,
    pub blases: &'a HashMap<Mesh, AccelerationStructure>,
}

pub struct Output {
    pub tlas: AccelerationStructure,
    pub probes: Buffer,
}

#[repr(C)]
struct ProbeData {
    sh: [f32; 9],
}

const MAX_INSTANCE_COUNT: u16 = 1024 * 32;

/// Pass toray-trace irradiance for probes dynamicall.
pub struct RayProbe {
    pipeline_layout: PipelineLayout,
    pipeline: RayTracingPipeline,
    shader_binding_table: ShaderBindingTable,

    tlas: AccelerationStructure,
    scratch: Buffer,
    main_buffer: Buffer,

    set: DescriptorSet,
    per_frame_sets: [DescriptorSet; 2],

    meshes: SparseDescriptors<Mesh>,
    albedo: SparseDescriptors<Texture>,
    normal: SparseDescriptors<Texture>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct ShaderInstance {
    transform: na::Matrix4<f32>,
    mesh: u32,
    albedo_sampler: u32,
    albedo_factor: [f32; 4],
    normal_sampler: u32,
    normal_factor: f32,
    anim: u32,
}

unsafe impl Zeroable for ShaderInstance {}
unsafe impl Pod for ShaderInstance {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct ShaderPointLight {
    position: [f32; 3],
    _pad0: f32,
    radiance: [f32; 3],
    _pad1: f32,
}

unsafe impl Zeroable for ShaderPointLight {}
unsafe impl Pod for ShaderPointLight {}

impl<'a> Pass<'a> for RayProbe {
    type Input = Input<'a>;
    type Output = Output;

    fn draw(
        &mut self,
        input: Input<'a>,
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<Output, Report> {
        todo!()
    }
}

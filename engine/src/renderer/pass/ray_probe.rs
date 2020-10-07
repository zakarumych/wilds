use {
    super::{Pass, SparseDescriptors},
    crate::renderer::{Context, Mesh, Texture},
    bumpalo::Bump,
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    hecs::World,
    illume::*,
    nalgebra as na,
    std::{
        collections::HashMap,
        mem::{align_of, size_of},
    },
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

const PROBES_EXTENT: Extent3d = Extent3d {
    width: 32,
    height: 32,
    depth: 32,
};

const PROBES_COUNT: u16 =
    PROBES_EXTENT.width * PROBES_EXTENT.height * PROBES_EXTENT.depth;

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

impl RayProbe {
    pub fn new(
        extent: Extent2d,
        ctx: &mut Context,
        blue_noise_buffer_256x256x128: Buffer,
    ) -> Result<Self, Report> {
        // Create pipeline.
        let set_layout = ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::empty(),
                bindings: vec![
                    // TLAS.
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: DescriptorType::AccelerationStructure,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Blue noise
                    DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: DescriptorType::StorageBuffer,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Indices
                    DescriptorSetLayoutBinding {
                        binding: 2,
                        ty: DescriptorType::StorageBuffer,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    // Vertex input.
                    DescriptorSetLayoutBinding {
                        binding: 3,
                        ty: DescriptorType::StorageBuffer,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    // Textures
                    DescriptorSetLayoutBinding {
                        binding: 4,
                        ty: DescriptorType::CombinedImageSampler,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    DescriptorSetLayoutBinding {
                        binding: 5,
                        ty: DescriptorType::CombinedImageSampler,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    // Output image
                    DescriptorSetLayoutBinding {
                        binding: 6,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                ],
            })?;

        let per_frame_set_layout = ctx.device.create_descriptor_set_layout(
            DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::empty(),
                bindings: vec![
                    // Globals
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: DescriptorType::UniformBuffer,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT
                            | ShaderStageFlags::MISS,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Scene
                    DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: DescriptorType::StorageBuffer,
                        count: 1,
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Lights
                    DescriptorSetLayoutBinding {
                        binding: 2,
                        ty: DescriptorType::StorageBuffer,
                        count: 1,
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Animated vertices
                    DescriptorSetLayoutBinding {
                        binding: 3,
                        ty: DescriptorType::StorageBuffer,
                        count: 1024,
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                    // Probes data
                    DescriptorSetLayoutBinding {
                        binding: 3,
                        ty: DescriptorType::StorageBuffer,
                        count: 1024,
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                    // New probes data
                    DescriptorSetLayoutBinding {
                        binding: 4,
                        ty: DescriptorType::StorageBuffer,
                        count: 1024,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                ],
            },
        )?;

        let pipeline_layout =
            ctx.create_pipeline_layout(PipelineLayoutInfo {
                sets: vec![set_layout.clone(), per_frame_set_layout.clone()],
                push_constants: Vec::new(),
            })?;

        let primary_rgen = RaygenShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("rt_prepass/viewport.rgen.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let primary_rmiss = MissShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("rt_prepass/primary.rmiss.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let primary_rchit = ClosestHitShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("rt_prepass/primary.rchit.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let diffuse_rmiss = MissShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("rt_prepass/diffuse.rmiss.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let diffuse_rchit = ClosestHitShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("rt_prepass/diffuse.rchit.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let shadow_rmiss = MissShader::with_main(
            ctx.create_shader_module(
                Spirv::new(include_bytes!("common/shadow.rmiss.spv").to_vec())
                    .into(),
            )?,
        );

        let pipeline =
            ctx.create_ray_tracing_pipeline(RayTracingPipelineInfo {
                shaders: vec![
                    primary_rgen.into(),
                    primary_rmiss.into(),
                    primary_rchit.into(),
                    diffuse_rmiss.into(),
                    diffuse_rchit.into(),
                    shadow_rmiss.into(),
                ],
                groups: vec![
                    RayTracingShaderGroupInfo::Raygen { raygen: 0 },
                    RayTracingShaderGroupInfo::Miss { miss: 1 },
                    RayTracingShaderGroupInfo::Miss { miss: 3 },
                    RayTracingShaderGroupInfo::Miss { miss: 5 },
                    RayTracingShaderGroupInfo::Triangles {
                        any_hit: None,
                        closest_hit: Some(2),
                    },
                    RayTracingShaderGroupInfo::Triangles {
                        any_hit: None,
                        closest_hit: Some(4),
                    },
                ],
                max_recursion_depth: 10,
                layout: pipeline_layout.clone(),
            })?;

        let shader_binding_table = ctx
            .create_ray_tracing_shader_binding_table(
                &pipeline,
                ShaderBindingTableInfo {
                    raygen: Some(0),
                    miss: &[1, 2, 3],
                    hit: &[4, 5],
                    callable: &[],
                },
            )?;

        tracing::trace!("RT pipeline created");

        // Creating TLAS.
        let tlas =
            ctx.create_acceleration_structure(AccelerationStructureInfo {
                level: AccelerationStructureLevel::Top,
                flags: AccelerationStructureFlags::empty(),
                geometries: vec![
                    AccelerationStructureGeometryInfo::Instances {
                        max_primitive_count: MAX_INSTANCE_COUNT.into(),
                    },
                ],
            })?;

        tracing::trace!("TLAS created");
        // Allocate scratch memory for TLAS building.
        let scratch =
            ctx.allocate_acceleration_structure_build_scratch(&tlas, false)?;

        tracing::trace!("TLAS scratch allocated");

        let globals_and_instances = ctx.create_buffer(BufferInfo {
            align: main_buffer_align(),
            size: main_buffer_size(),
            usage: BufferUsage::UNIFORM
                | BufferUsage::STORAGE
                | BufferUsage::RAY_TRACING
                | BufferUsage::SHADER_DEVICE_ADDRESS,
            memory: MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::FAST_DEVICE_ACCESS,
        })?;

        tracing::trace!("Globals and instances buffer created");

        // Image matching surface extent.
        let output_albedo_image = ctx.create_image(ImageInfo {
            extent: extent.into(),
            format: Format::RGBA8Unorm,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
            memory: MemoryUsageFlags::empty(),
        })?;

        // View for whole image
        let output_view =
            ctx.create_image_view(ImageViewInfo::new(output_image.clone()))?;

        tracing::trace!("Feature images created");

        let set = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let per_frame_set0 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: per_frame_set_layout.clone(),
        })?;

        let per_frame_set1 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: per_frame_set_layout.clone(),
        })?;

        tracing::trace!("Descriptor sets created");

        ctx.update_descriptor_sets(
            &[
                WriteDescriptorSet {
                    set: &set,
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::AccelerationStructure(
                        std::slice::from_ref(&tlas),
                    ),
                },
                WriteDescriptorSet {
                    set: &set,
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        blue_noise_buffer_256x256x128.clone(),
                        0,
                        blue_noise_buffer_256x256x128.info().size,
                    )]),
                },
                WriteDescriptorSet {
                    set: &set,
                    binding: 7,
                    element: 0,
                    descriptors: Descriptors::StorageImage(&[(
                        output_view.clone(),
                        Layout::General,
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set0,
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::UniformBuffer(&[(
                        globals_and_instances.clone(),
                        globals_offset(0),
                        globals_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set1,
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::UniformBuffer(&[(
                        globals_and_instances.clone(),
                        globals_offset(1),
                        globals_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set0,
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        globals_and_instances.clone(),
                        instances_offset(0),
                        instances_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set1,
                    binding: 1,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        globals_and_instances.clone(),
                        instances_offset(1),
                        instances_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set0,
                    binding: 2,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        globals_and_instances.clone(),
                        pointlight_offset(0),
                        pointlight_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &per_frame_set1,
                    binding: 2,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        globals_and_instances.clone(),
                        pointlight_offset(1),
                        pointlight_size(),
                    )]),
                },
            ],
            &[],
        );

        Ok(RayProbe {
            pipeline_layout,
            pipeline,
            shader_binding_table,
            tlas,
            scratch,
            main_buffer,
            set,
            per_frame_sets: [per_frame_set0, per_frame_set1],
            meshes: SparseDescriptors::new(),
            albedo: SparseDescriptors::new(),
            normal: SparseDescriptors::new(),
        })
    }
}

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

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GlobalsCamera {
    view: na::Matrix4<f32>,
    proj: na::Matrix4<f32>,
    iview: na::Matrix4<f32>,
    iproj: na::Matrix4<f32>,
}

unsafe impl Zeroable for GlobalsCamera {}
unsafe impl Pod for GlobalsCamera {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GlobalsDirLight {
    dir: [f32; 3],
    _pad0: f32,
    rad: [f32; 3],
    _pad1: f32,
}

unsafe impl Zeroable for GlobalsDirLight {}
unsafe impl Pod for GlobalsDirLight {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Globals {
    dirlight: GlobalsDirLight,
    skylight: na::Vector3<f32>,
    _pad0: f32,
    plights: u32,
    frame: u32,
    shadow_rays: u32,
    diffuse_rays: u32,
    extent: na::Vector3<f32>,
    _pad1: f32,
    offset: na::Vector3<f32>,
    _pad2: f32,
}

unsafe impl Zeroable for Globals {}
unsafe impl Pod for Globals {}

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
    _pad: [f32; 3],
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

const fn globals_size() -> u64 {
    size_of::<Globals>() as u64
}

fn globals_offset(frame: u32) -> u64 {
    u64::from(frame) * align_up(255, globals_size()).unwrap()
}

fn globals_end(frame: u32) -> u64 {
    globals_offset(frame) + globals_size()
}

const fn instances_size() -> u64 {
    size_of::<[ShaderInstance; MAX_INSTANCE_COUNT as usize]>() as u64
}

fn instances_offset(frame: u32) -> u64 {
    align_up(255, globals_end(1)).unwrap()
        + u64::from(frame) * align_up(255, instances_size()).unwrap()
}

fn instances_end(frame: u32) -> u64 {
    instances_offset(frame) + instances_size()
}

const fn pointlight_size() -> u64 {
    size_of::<[ShaderPointLight; 32]>() as u64
}

fn pointlight_offset(frame: u32) -> u64 {
    align_up(255, instances_end(1)).unwrap()
        + u64::from(frame) * align_up(255, pointlight_size()).unwrap()
}

fn pointlight_end(frame: u32) -> u64 {
    pointlight_offset(frame) + pointlight_size()
}

const fn probes_size() -> u64 {
    size_of::<[ProbeData; PROBES_COUNT as usize]>() as u64
}

fn probes_offset(frame: u32) -> u64 {
    align_up(255, instances_end(1)).unwrap()
        + u64::from(frame) * align_up(255, probes_size()).unwrap()
}

fn probes_end(frame: u32) -> u64 {
    probes_offset(frame) + probes_size()
}

const fn acc_instances_size() -> u64 {
    size_of::<[AccelerationStructureInstance; MAX_INSTANCE_COUNT as usize]>()
        as u64
}

fn acc_instances_offset(frame: u32) -> u64 {
    align_up(255, probes_end(1)).unwrap()
        + u64::from(frame) * align_up(255, acc_instances_size()).unwrap()
}

fn acc_instances_end(frame: u32) -> u64 {
    acc_instances_offset(frame) + acc_instances_size()
}

const fn main_buffer_align() -> u64 {
    255
}

fn main_buffer_size() -> u64 {
    acc_instances_end(1)
}

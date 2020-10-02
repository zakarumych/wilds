use {
    super::{Pass, SparseDescriptors},
    crate::{
        animate::Pose,
        light::{DirectionalLight, PointLight, SkyLight},
        renderer::{
            ray_tracing_transform_matrix_from_nalgebra, Context, Mesh,
            PoseMesh, PositionNormalTangent3dUV, Renderable, Texture,
            VertexType,
        },
        scene::Global3,
    },
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    eyre::ensure,
    hecs::World,
    illume::*,
    nalgebra as na,
    std::{collections::HashMap, convert::TryFrom as _, mem::size_of},
};

const MAX_INSTANCE_COUNT: u16 = 1024 * 32;

pub struct Input<'a> {
    pub extent: Extent2d,
    pub camera_global: Global3,
    pub camera_projection: na::Projective3<f32>,
    pub blases: &'a HashMap<Mesh, AccelerationStructure>,
}

pub struct Output {
    pub tlas: AccelerationStructure,
    pub albedo: Image,
    pub normal_depth: Image,
    pub emissive: Image,
    pub direct: Image,
    pub diffuse: Image,
}

pub struct RtPrepass {
    pipeline_layout: PipelineLayout,
    pipeline: RayTracingPipeline,
    shader_binding_table: ShaderBindingTable,

    tlas: AccelerationStructure,
    scratch: Buffer,
    globals_and_instances: Buffer,

    set: DescriptorSet,
    per_frame_sets: [DescriptorSet; 2],

    meshes: SparseDescriptors<Mesh>,
    albedo: SparseDescriptors<Texture>,
    normal: SparseDescriptors<Texture>,

    output_albedo_image: Image,
    output_normal_depth_image: Image,
    output_emissive_image: Image,
    output_direct_image: Image,
    output_diffuse_image: Image,
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

impl RtPrepass {
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
                    // G-Buffer
                    // Albedo
                    DescriptorSetLayoutBinding {
                        binding: 6,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // normal-depth
                    DescriptorSetLayoutBinding {
                        binding: 7,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // emissive
                    DescriptorSetLayoutBinding {
                        binding: 8,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // direct
                    DescriptorSetLayoutBinding {
                        binding: 9,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // diffuse
                    DescriptorSetLayoutBinding {
                        binding: 10,
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
                    include_bytes!("rt_prepass/primary.rgen.spv").to_vec(),
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
                Spirv::new(
                    include_bytes!("rt_prepass/shadow.rmiss.spv").to_vec(),
                )
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
            align: globals_and_instances_align(),
            size: globals_and_instances_size(),
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
        let output_albedo_view = ctx.create_image_view(ImageViewInfo::new(
            output_albedo_image.clone(),
        ))?;

        let output_normal_depth_image = ctx.create_image(ImageInfo {
            extent: extent.into(),
            format: Format::RGBA32Sfloat,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
            memory: MemoryUsageFlags::empty(),
        })?;

        // View for whole image
        let output_normal_depth_view = ctx.create_image_view(
            ImageViewInfo::new(output_normal_depth_image.clone()),
        )?;

        let output_emissive_image = ctx.create_image(ImageInfo {
            extent: extent.into(),
            format: Format::RGBA32Sfloat,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
            memory: MemoryUsageFlags::empty(),
        })?;

        // View for whole image
        let output_emissive_view = ctx.create_image_view(
            ImageViewInfo::new(output_emissive_image.clone()),
        )?;

        let output_direct_image = ctx.create_image(ImageInfo {
            extent: extent.into(),
            format: Format::RGBA32Sfloat,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
            memory: MemoryUsageFlags::empty(),
        })?;

        // View for whole image
        let output_direct_view = ctx.create_image_view(ImageViewInfo::new(
            output_direct_image.clone(),
        ))?;

        let output_diffuse_image = ctx.create_image(ImageInfo {
            extent: extent.into(),
            format: Format::RGBA32Sfloat,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
            memory: MemoryUsageFlags::empty(),
        })?;

        // View for whole image
        let output_diffuse_view = ctx.create_image_view(ImageViewInfo::new(
            output_diffuse_image.clone(),
        ))?;

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
                    binding: 6,
                    element: 0,
                    descriptors: Descriptors::StorageImage(&[
                        (output_albedo_view.clone(), Layout::General),
                        (output_normal_depth_view.clone(), Layout::General),
                        (output_emissive_view.clone(), Layout::General),
                        (output_direct_view.clone(), Layout::General),
                        (output_diffuse_view.clone(), Layout::General),
                    ]),
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

        Ok(RtPrepass {
            pipeline_layout,
            pipeline,
            shader_binding_table,
            tlas,
            scratch,
            globals_and_instances,
            set,
            per_frame_sets: [per_frame_set0, per_frame_set1],
            output_albedo_image,
            output_normal_depth_image,
            output_emissive_image,
            output_direct_image,
            output_diffuse_image,
            meshes: SparseDescriptors::new(),
            albedo: SparseDescriptors::new(),
            normal: SparseDescriptors::new(),
        })
    }
}

impl<'a> Pass<'a> for RtPrepass {
    type Input = Input<'a>;
    type Output = Output;

    #[tracing::instrument(skip(
        self, input, frame, wait, signal, fence, ctx, world, bump
    ))]
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
        tracing::trace!("RtPrepass::draw");

        let findex = (frame & 1) as u32;

        assert_eq!(self.output_albedo_image.info().extent, input.extent.into());

        // https://microsoft.github.io/DirectX-Specs/d3d/Raytracing.html#general-tips-for-building-acceleration-structures
        //
        // > Rebuild top-level acceleration structure every frame
        //   Only updating instead of rebuilding is rarely the right thing to
        // do.   Rebuilds for a few thousand instances are very fast,
        //   and having a good quality top-level acceleration structure can have
        // a significant payoff   (bad quality has a higher cost further
        // up in the tree).
        let mut instances = BVec::new_in(bump);
        let mut acc_instances = BVec::new_in(bump);
        let mut anim_vertices_descriptors = BVec::new_in(bump);

        let mut writes = BVec::new_in(bump);

        let mut encoder = ctx.queue.create_encoder()?;

        let mut query = world.query::<(
            &Renderable,
            &Global3,
            Option<&Pose>,
            Option<&PoseMesh>,
        )>();

        tracing::trace!("Query all renderable");

        for (entity, (renderable, global, pose, pose_mesh)) in query.iter() {
            if let Some(blas) = input.blases.get(&renderable.mesh) {
                let blas_address =
                    ctx.get_acceleration_structure_device_address(blas);

                // let m = match renderable.transform {
                //     Some(t) => global.to_homogeneous() * t,
                //     None => global.to_homogeneous(),
                // };

                let m = global.to_homogeneous();

                let (mut mesh_index, new) =
                    self.meshes.index(renderable.mesh.clone());
                if new {
                    let vectors = renderable
                        .mesh
                        .bindings()
                        .iter()
                        .find(|binding| {
                            binding.layout
                                == PositionNormalTangent3dUV::layout()
                        })
                        .unwrap();

                    let vectors_buffer = vectors.buffer.clone();
                    let vectors_offset = vectors.offset;
                    let vectors_size: u64 = vectors.layout.stride as u64
                        * renderable.mesh.vertex_count() as u64;

                    let indices = renderable.mesh.indices().unwrap();
                    let indices_buffer = indices.buffer.clone();
                    let indices_offset = indices.offset;
                    let indices_size: u64 = indices.index_type.size() as u64
                        * renderable.mesh.count() as u64;

                    assert_eq!(vectors_offset & 15, 0);
                    assert_eq!(indices_offset & 15, 0);

                    // FIXME: Leak
                    let indices_desc = Descriptors::StorageBuffer(bump.alloc(
                        [(indices_buffer, indices_offset, indices_size)],
                    ));

                    let vectors_desc = Descriptors::StorageBuffer(bump.alloc(
                        [(vectors_buffer, vectors_offset, vectors_size)],
                    ));

                    writes.push(WriteDescriptorSet {
                        set: &self.set,
                        binding: 2,
                        element: mesh_index,
                        descriptors: indices_desc,
                    });

                    writes.push(WriteDescriptorSet {
                        set: &self.set,
                        binding: 3,
                        element: mesh_index,
                        descriptors: vectors_desc,
                    });
                }

                let anim = if let (Some(_), Some(pose_mesh)) = (pose, pose_mesh)
                {
                    let vectors = pose_mesh
                        .bindings()
                        .iter()
                        .find(|binding| {
                            binding.layout
                                == PositionNormalTangent3dUV::layout()
                        })
                        .unwrap();

                    let vectors_buffer = vectors.buffer.clone();
                    let vectors_offset = vectors.offset;
                    let vectors_size: u64 = vectors.layout.stride as u64
                        * renderable.mesh.vertex_count() as u64;

                    mesh_index = anim_vertices_descriptors.len() as u32;

                    anim_vertices_descriptors.push((
                        vectors_buffer,
                        vectors_offset,
                        vectors_size,
                    ));

                    let blas = renderable.mesh.build_pose_triangles_blas(
                        pose_mesh,
                        &mut encoder,
                        &ctx.device,
                        bump,
                    )?;

                    // FIXME: blas leak
                    let blas_address = ctx
                        .device
                        .get_acceleration_structure_device_address(&blas);

                    acc_instances.push(
                        AccelerationStructureInstance::new(blas_address)
                            .with_transform(
                                ray_tracing_transform_matrix_from_nalgebra(&m),
                            ),
                    );

                    true
                } else {
                    acc_instances.push(
                        AccelerationStructureInstance::new(blas_address)
                            .with_transform(
                                ray_tracing_transform_matrix_from_nalgebra(&m),
                            ),
                    );
                    false
                };

                let albedo_index = if let Some(albedo) =
                    &renderable.material.albedo
                {
                    let (albedo_index, new) = self.albedo.index(albedo.clone());

                    if new {
                        let descriptors =
                            Descriptors::CombinedImageSampler(bump.alloc([(
                                albedo.image.clone(),
                                Layout::General,
                                albedo.sampler.clone(),
                            )]));
                        writes.push(WriteDescriptorSet {
                            set: &self.set,
                            binding: 4,
                            element: albedo_index,
                            descriptors,
                        });
                    }

                    albedo_index + 1
                } else {
                    0
                };

                let normal_index = if let Some(normal) =
                    &renderable.material.normal
                {
                    let (normal_index, new) = self.normal.index(normal.clone());

                    if new {
                        let descriptors =
                            Descriptors::CombinedImageSampler(bump.alloc([(
                                normal.image.clone(),
                                Layout::General,
                                normal.sampler.clone(),
                            )]));
                        writes.push(WriteDescriptorSet {
                            set: &self.set,
                            binding: 5,
                            element: normal_index,
                            descriptors,
                        });
                    }

                    normal_index + 1
                } else {
                    0
                };

                instances.push(ShaderInstance {
                    transform: m,
                    mesh: mesh_index,
                    albedo_sampler: albedo_index,
                    normal_sampler: normal_index,
                    albedo_factor: {
                        let [r, g, b, a] = renderable.material.albedo_factor;
                        [
                            r.into_inner(),
                            g.into_inner(),
                            b.into_inner(),
                            a.into_inner(),
                        ]
                    },
                    normal_factor: renderable
                        .material
                        .normal_factor
                        .into_inner(),
                    anim: anim as u32,
                });
            } else {
                tracing::error!("Missing BLAS for mesh @ {:?}", entity);
            }
        }

        if !anim_vertices_descriptors.is_empty() {
            writes.push(WriteDescriptorSet {
                set: &self.per_frame_sets[findex as usize],
                binding: 3,
                element: 0,
                descriptors: Descriptors::StorageBuffer(
                    &anim_vertices_descriptors,
                ),
            });
        }

        tracing::trace!("Update descriptors");

        ctx.update_descriptor_sets(&writes, &[]);

        drop(writes);

        ensure!(
            instances.len() <= MAX_INSTANCE_COUNT.into(),
            "Too many instances"
        );

        ensure!(
            acc_instances.len() <= MAX_INSTANCE_COUNT.into(),
            "Too many instances"
        );

        ensure!(u32::try_from(instances.len()).is_ok(), "Too many instances");

        tracing::trace!("Build TLAS");

        // Sync BLAS and TLAS builds.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
        );

        let infos = bump.alloc([AccelerationStructureBuildGeometryInfo {
            src: None,
            dst: self.tlas.clone(),
            geometries: bump.alloc([
                AccelerationStructureGeometry::Instances {
                    flags: GeometryFlags::OPAQUE,
                    data: ctx
                        .get_buffer_device_address(&self.globals_and_instances)
                        .unwrap()
                        .offset(acc_instances_offset(findex)),
                    primitive_count: instances.len() as u32,
                },
            ]),
            scratch: ctx.get_buffer_device_address(&self.scratch).unwrap(),
        }]);

        encoder.build_acceleration_structure(infos);

        tracing::trace!("Update Globals");

        ctx.write_memory(
            &self.globals_and_instances,
            acc_instances_offset(findex),
            &acc_instances,
        )?;

        tracing::trace!("Update Globals");

        ctx.write_memory(
            &self.globals_and_instances,
            instances_offset(findex),
            &instances,
        )?;

        let mut pointlights: BVec<ShaderPointLight> =
            BVec::with_capacity_in(32, bump);
        pointlights.extend(
            world
                .query::<(&PointLight, &Global3)>()
                .iter()
                .map(|(_, (pl, global))| ShaderPointLight {
                    position: global.iso.translation.vector.into(),
                    radiance: pl.radiance,
                    _pad0: 0.0,
                    _pad1: 0.0,
                })
                .take(32),
        );

        tracing::trace!("Update Globals");

        ctx.write_memory(
            &self.globals_and_instances,
            pointlight_offset(findex),
            &pointlights,
        )?;

        let dirlight = world
            .query::<&DirectionalLight>()
            .iter()
            .next()
            .map(|(_, dl)| GlobalsDirLight {
                rad: dl.radiance,
                dir: dl.direction.into(),
                _pad0: 0.0,
                _pad1: 0.0,
            })
            .unwrap_or(GlobalsDirLight {
                rad: [0.0; 3],
                dir: [0.0; 3],
                _pad0: 0.0,
                _pad1: 0.0,
            });

        let skylight = world
            .query::<&SkyLight>()
            .iter()
            .next()
            .map(|(_, sl)| sl.radiance)
            .unwrap_or_default();

        let globals = Globals {
            camera: GlobalsCamera {
                view: input.camera_global.to_homogeneous(),
                // iview: input.camera_global.inverse().to_homogeneous(),
                iview: na::Matrix4::identity(),
                proj: input.camera_projection.to_homogeneous(),
                iproj: input.camera_projection.inverse().to_homogeneous(),
            },
            dirlight,
            skylight,
            plights: pointlights.len() as u32,
            // frame: frame as u32,
            frame: 0,
            shadow_rays: 4,
            diffuse_rays: 4,
        };

        tracing::trace!("Update Globals");

        ctx.write_memory(
            &self.globals_and_instances,
            globals_offset(findex),
            std::slice::from_ref(&globals),
        )?;

        tracing::trace!("Trace rays");

        encoder.bind_ray_tracing_pipeline(&self.pipeline);

        encoder.bind_ray_tracing_descriptor_sets(
            &self.pipeline_layout,
            0,
            bump.alloc([
                self.set.clone(),
                self.per_frame_sets[findex as usize].clone(),
            ]),
            &[],
        );

        // Sync storage image access from last frame.
        let images = [
            ImageLayoutTransition::initialize_whole(
                &self.output_albedo_image,
                Layout::General,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &self.output_normal_depth_image,
                Layout::General,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &self.output_emissive_image,
                Layout::General,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &self.output_direct_image,
                Layout::General,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &self.output_diffuse_image,
                Layout::General,
            )
            .into(),
        ];

        encoder.image_barriers(
            PipelineStageFlags::FRAGMENT_SHADER, // FIXME: Compure barrier.
            PipelineStageFlags::RAY_TRACING_SHADER,
            &images,
        );

        // Sync TLAS build with ray-tracing shader where it will be used.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
            PipelineStageFlags::RAY_TRACING_SHADER,
        );

        // Perform ray-trace operation.
        encoder.trace_rays(&self.shader_binding_table, input.extent.into_3d());

        // Sync storage image access from last frame.
        let images = [
            ImageLayoutTransition::transition_whole(
                &self.output_albedo_image,
                Layout::General..Layout::ShaderReadOnlyOptimal,
            )
            .into(),
            ImageLayoutTransition::transition_whole(
                &self.output_normal_depth_image,
                Layout::General..Layout::ShaderReadOnlyOptimal,
            )
            .into(),
            ImageLayoutTransition::transition_whole(
                &self.output_emissive_image,
                Layout::General..Layout::ShaderReadOnlyOptimal,
            )
            .into(),
            ImageLayoutTransition::transition_whole(
                &self.output_direct_image,
                Layout::General..Layout::ShaderReadOnlyOptimal,
            )
            .into(),
            ImageLayoutTransition::transition_whole(
                &self.output_diffuse_image,
                Layout::General..Layout::ShaderReadOnlyOptimal,
            )
            .into(),
        ];

        encoder.image_barriers(
            PipelineStageFlags::RAY_TRACING_SHADER,
            PipelineStageFlags::FRAGMENT_SHADER,
            &images,
        );

        let cbuf = encoder.finish();

        tracing::trace!("Submitting");

        ctx.queue.submit(wait, cbuf, signal, fence);

        Ok(Output {
            albedo: self.output_albedo_image.clone(),
            normal_depth: self.output_normal_depth_image.clone(),
            emissive: self.output_emissive_image.clone(),
            direct: self.output_direct_image.clone(),
            diffuse: self.output_diffuse_image.clone(),
            tlas: self.tlas.clone(),
        })
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
    camera: GlobalsCamera,
    dirlight: GlobalsDirLight,
    skylight: [f32; 3],
    plights: u32,
    frame: u32,
    shadow_rays: u32,
    diffuse_rays: u32,
}

unsafe impl Zeroable for Globals {}
unsafe impl Pod for Globals {}

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

const fn acc_instances_size() -> u64 {
    size_of::<[AccelerationStructureInstance; MAX_INSTANCE_COUNT as usize]>()
        as u64
}

fn acc_instances_offset(frame: u32) -> u64 {
    align_up(255, pointlight_end(1)).unwrap()
        + u64::from(frame) * align_up(255, acc_instances_size()).unwrap()
}

fn acc_instances_end(frame: u32) -> u64 {
    acc_instances_offset(frame) + acc_instances_size()
}

const fn globals_and_instances_align() -> u64 {
    255
}

fn globals_and_instances_size() -> u64 {
    acc_instances_end(1)
}

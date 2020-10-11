use {
    super::{Pass, SparseDescriptors},
    crate::{
        animate::Pose,
        light::{DirectionalLight, PointLight, SkyLight},
        renderer::{
            ray_tracing_transform_matrix_from_nalgebra, Context, Mesh,
            PoseMesh, PositionNormalTangent3dUV, Renderable, Texture,
            VertexType as _,
        },
        scene::Global3,
        util::BumpaloCellList,
    },
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    eyre::{ensure, Report},
    hecs::World,
    illume::*,
    nalgebra as na,
    std::{
        collections::HashMap,
        convert::TryFrom as _,
        mem::{align_of, size_of},
    },
};

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub probes_extent: Extent3d,
    pub probes_dimensions: [f32; 3],
    pub probes_offset: [f32; 3],
}

impl Config {
    pub const fn new() -> Self {
        Config {
            probes_extent: Extent3d {
                width: 33,
                height: 33,
                depth: 33,
            },
            probes_dimensions: [32.0; 3],
            probes_offset: [-16.0; 3],
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Input<'a> {
    pub extent: Extent2d,
    pub camera_global: Global3,
    pub camera_projection: na::Projective3<f32>,
    pub blases: &'a HashMap<Mesh, AccelerationStructure>,
}

pub struct Output {
    pub tlas: AccelerationStructure,
    pub probes: Buffer,
    pub output_image: Image,
}

#[repr(C)]
struct ProbeData {
    sh: [f32; 4 * 9],
}

const MAX_INSTANCE_COUNT: u16 = 1024 * 32;

/// Pass toray-trace irradiance for probes dynamicall.
pub struct RayProbe {
    pipeline_layout: PipelineLayout,
    pipeline: RayTracingPipeline,
    probes_binding_table: ShaderBindingTable,
    viewport_binding_table: ShaderBindingTable,

    tlas: AccelerationStructure,
    scratch: Buffer,
    globals_and_instances: Buffer,
    probes_buffer: Option<Buffer>,
    probes_bound: [bool; 2],

    output_image: Option<ImageView>,
    output_bound: [bool; 2],

    set: DescriptorSet,
    per_frame_sets: [DescriptorSet; 2],

    meshes: SparseDescriptors<Mesh>,
    albedo: SparseDescriptors<Texture>,
    normal: SparseDescriptors<Texture>,
}

impl RayProbe {
    pub fn new(
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

        let probes_rgen = RaygenShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("ray_probe/probes.rgen.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let viewport_rgen = RaygenShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("ray_probe/viewport.rgen.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let primary_rmiss = MissShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("ray_probe/primary.rmiss.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let primary_rchit = ClosestHitShader::with_main(
            ctx.create_shader_module(
                Spirv::new(
                    include_bytes!("ray_probe/primary.rchit.spv").to_vec(),
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
                    probes_rgen.into(),
                    viewport_rgen.into(),
                    primary_rmiss.into(),
                    primary_rchit.into(),
                    shadow_rmiss.into(),
                ],
                groups: vec![
                    RayTracingShaderGroupInfo::Raygen { raygen: 0 },
                    RayTracingShaderGroupInfo::Raygen { raygen: 1 },
                    RayTracingShaderGroupInfo::Miss { miss: 2 },
                    RayTracingShaderGroupInfo::Miss { miss: 4 },
                    RayTracingShaderGroupInfo::Triangles {
                        any_hit: None,
                        closest_hit: Some(3),
                    },
                ],
                max_recursion_depth: 10,
                layout: pipeline_layout.clone(),
            })?;

        let probes_binding_table = ctx
            .create_ray_tracing_shader_binding_table(
                &pipeline,
                ShaderBindingTableInfo {
                    raygen: Some(0),
                    miss: &[2, 3],
                    hit: &[4],
                    callable: &[],
                },
            )?;

        let viewport_binding_table = ctx
            .create_ray_tracing_shader_binding_table(
                &pipeline,
                ShaderBindingTableInfo {
                    raygen: Some(1),
                    miss: &[2, 3],
                    hit: &[4],
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

        tracing::trace!("Main buffer created");

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
            probes_binding_table,
            viewport_binding_table,
            tlas,
            scratch,
            globals_and_instances,
            probes_buffer: None,
            probes_bound: [false; 2],
            output_image: None,
            output_bound: [false; 2],
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

        let config = world
            .query::<&Config>()
            .iter()
            .next()
            .map(|(_, c)| *c)
            .unwrap_or_default();

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
        let storage_buffers = BumpaloCellList::new();
        let storage_images = BumpaloCellList::new();
        let bind_descriptor_sets;

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
                    _pad: [0.0; 3],
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

        tracing::trace!("Update probes buffer");

        let probes_buffer = match &mut self.probes_buffer {
            Some(probes_buffer)
                if probes_buffer.info().size
                    == probes_buffer_size(&config.probes_extent) =>
            {
                probes_buffer.clone()
            }

            slot => {
                self.probes_bound = [false; 2];

                let buffer = ctx.create_buffer(BufferInfo {
                    align: probes_buffer_align(),
                    size: probes_buffer_size(&config.probes_extent),
                    usage: BufferUsage::STORAGE,
                    memory: MemoryUsageFlags::empty(),
                })?;

                *slot = Some(buffer.clone());
                buffer
            }
        };

        if !self.probes_bound[findex as usize] {
            writes.push(WriteDescriptorSet {
                set: &self.per_frame_sets[findex as usize],
                binding: 4,
                element: 0,
                descriptors: Descriptors::StorageBuffer(std::slice::from_ref(
                    storage_buffers.push_in(
                        (
                            probes_buffer.clone(),
                            probes_offset(findex, &config.probes_extent),
                            probes_size(&config.probes_extent),
                        ),
                        bump,
                    ),
                )),
            });
            writes.push(WriteDescriptorSet {
                set: &self.per_frame_sets[findex as usize],
                binding: 5,
                element: 0,
                descriptors: Descriptors::StorageBuffer(std::slice::from_ref(
                    storage_buffers.push_in(
                        (
                            probes_buffer.clone(),
                            probes_offset(1 - findex, &config.probes_extent),
                            probes_size(&config.probes_extent),
                        ),
                        bump,
                    ),
                )),
            });
            self.probes_bound[findex as usize] = true;
        }

        tracing::trace!("Update output image");

        let output_image = match &mut self.output_image {
            Some(output_image) => output_image.clone(),

            slot => {
                self.output_bound = [false; 2];

                let image = ctx.create_image(ImageInfo {
                    extent: input.extent.into(),
                    format: Format::RGBA8Unorm,
                    levels: 1,
                    layers: 1,
                    samples: Samples1,
                    usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
                    memory: MemoryUsageFlags::empty(),
                })?;

                let view = ctx.create_image_view(ImageViewInfo::new(image))?;

                *slot = Some(view.clone());
                view
            }
        };

        if !self.output_bound[findex as usize] {
            writes.push(WriteDescriptorSet {
                set: &self.per_frame_sets[findex as usize],
                binding: 6,
                element: 0,
                descriptors: Descriptors::StorageImage(std::slice::from_ref(
                    storage_images
                        .push_in((output_image.clone(), Layout::General), bump),
                )),
            });
            self.output_bound[findex as usize] = true;
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
            shadow_rays: 1,
            diffuse_rays: 1,
            probes_dimensions: config.probes_dimensions.into(),
            probes_offset: config.probes_offset.into(),
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        tracing::trace!("Update Globals");

        ctx.write_memory(
            &self.globals_and_instances,
            globals_offset(findex),
            std::slice::from_ref(&globals),
        )?;

        tracing::trace!("Trace rays");

        encoder.bind_ray_tracing_pipeline(&self.pipeline);

        bind_descriptor_sets = [
            self.set.clone(),
            self.per_frame_sets[findex as usize].clone(),
        ];

        encoder.bind_ray_tracing_descriptor_sets(
            &self.pipeline_layout,
            0,
            &bind_descriptor_sets,
            &[],
        );

        // Sync TLAS build with ray-tracing shader where it will be used.
        // Sync previous probes buffer access with probes query.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD
                | PipelineStageFlags::RAY_TRACING_SHADER, // FIXME: Compure barrier.
            PipelineStageFlags::RAY_TRACING_SHADER,
        );

        // Trace probes.
        encoder.trace_rays(&self.probes_binding_table, config.probes_extent);

        let images = [ImageLayoutTransition::initialize_whole(
            &output_image.info().image,
            Layout::General,
        )
        .into()];

        // Sync probe query and probe reads.
        // Sync output image with previous access.
        encoder.image_barriers(
            PipelineStageFlags::RAY_TRACING_SHADER,
            PipelineStageFlags::RAY_TRACING_SHADER,
            &images,
        );

        // Trace viewport.
        encoder
            .trace_rays(&self.viewport_binding_table, input.extent.into_3d());

        // Sync storage image with presentation.
        let images = [ImageLayoutTransition::transition_whole(
            &output_image.info().image,
            Layout::General..Layout::TransferSrcOptimal,
        )
        .into()];

        encoder.image_barriers(
            PipelineStageFlags::RAY_TRACING_SHADER,
            PipelineStageFlags::BOTTOM_OF_PIPE,
            &images,
        );

        let cbuf = encoder.finish();

        tracing::trace!("Submitting");

        ctx.queue.submit(wait, cbuf, signal, fence);

        Ok(Output {
            output_image: output_image.info().image.clone(),
            probes: probes_buffer,
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
    _pad0: f32,
    plights: u32,
    frame: u32,
    shadow_rays: u32,
    diffuse_rays: u32,
    probes_dimensions: [f32; 3],
    _pad1: f32,
    probes_offset: [f32; 3],
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

const fn probes_size(extent: &Extent3d) -> u64 {
    let count =
        extent.width as u64 * extent.height as u64 * extent.depth as u64;
    size_of::<ProbeData>() as u64 * count
}

fn probes_offset(frame: u32, extent: &Extent3d) -> u64 {
    u64::from(frame) * align_up(255, probes_size(extent)).unwrap()
}

fn probes_end(frame: u32, extent: &Extent3d) -> u64 {
    probes_offset(frame, extent) + probes_size(extent)
}

const fn probes_buffer_align() -> u64 {
    255
}

fn probes_buffer_size(extent: &Extent3d) -> u64 {
    probes_end(1, extent)
}

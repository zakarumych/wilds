use {
    super::Pass,
    crate::{
        clocks::ClockIndex,
        light::{DirectionalLight, SkyLight},
        renderer::{
            Context, Mesh, PositionNormalTangent3dUV, Renderable, Texture,
            VertexType,
        },
    },
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    eyre::ensure,
    fastbitset::BitSet,
    hecs::World,
    illume::*,
    std::{
        collections::hash_map::{Entry, HashMap},
        convert::TryFrom as _,
        hash::Hash,
        mem::size_of,
    },
    ultraviolet::{Isometry3, Mat4, Vec3},
};

const MAX_INSTANCE_COUNT: u16 = 1024;

pub struct Input<'a> {
    pub extent: Extent2d,
    pub camera_transform: Mat4,
    pub camera_projection: Mat4,
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

struct SparseDescriptors<T> {
    resources: HashMap<T, u32>,
    bitset: BitSet,
    next: u32,
}

impl<T> SparseDescriptors<T>
where
    T: Hash + Eq,
{
    fn new() -> Self {
        SparseDescriptors {
            resources: HashMap::new(),
            bitset: BitSet::new(),
            next: 0,
        }
    }

    fn index(&mut self, resource: T) -> (u32, bool) {
        match self.resources.entry(resource) {
            Entry::Occupied(entry) => (*entry.get(), false),
            Entry::Vacant(entry) => {
                if let Some(index) = self.bitset.find_set() {
                    self.bitset.unset(index);
                    (*entry.insert(index as u32), true)
                } else {
                    self.next += 1;
                    (*entry.insert(self.next - 1), true)
                }
            }
        }
    }

    fn _remove(&mut self, resource: &T) -> Option<u32> {
        if let Some(value) = self.resources.remove(resource) {
            if value == self.next - 1 {
                self.next -= 1;
            } else {
                self.bitset.set(value as usize);
            }
            Some(value)
        } else {
            None
        }
    }
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
    transform: Mat4,
    mesh: u32,
    albedo_sampler: u32,
    albedo_factor: [f32; 4],
    normal_sampler: u32,
    normal_factor: f32,
}

unsafe impl Zeroable for ShaderInstance {}
unsafe impl Pod for ShaderInstance {}

impl RtPrepass {
    pub fn new(
        extent: Extent2d,
        ctx: &mut Context,
        blue_noise_buffer_64x64x64: Buffer,
    ) -> Result<Self, Report> {
        // Create pipeline.
        let set_layout = ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::UPDATE_AFTER_BIND_POOL,
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
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                ],
            },
        )?;

        let pipeline_layout =
            ctx.create_pipeline_layout(PipelineLayoutInfo {
                sets: vec![set_layout.clone(), per_frame_set_layout.clone()],
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
                        blue_noise_buffer_64x64x64.clone(),
                        0,
                        blue_noise_buffer_64x64x64.info().size,
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

    fn draw(
        &mut self,
        input: Input<'a>,
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        _clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<Output, Report> {
        let fid = (frame % 2) as u32;

        assert_eq!(self.output_albedo_image.info().extent, input.extent.into());

        // https://microsoft.github.io/DirectX-Specs/d3d/Raytracing.html#general-tips-for-building-acceleration-structures
        //
        // > Rebuild top-level acceleration structure every frame
        //   Only updating instead of rebuilding is rarely the right thing to
        // do.   Rebuilds for a few thousand instances are very fast,
        //   and having a good quality top-level acceleration structure can have
        // a significant payoff   (bad quality has a higher cost further
        // up in the tree).
        let mut instances: Vec<ShaderInstance> = Vec::new();
        let mut acc_instances: Vec<AccelerationStructureInstance> = Vec::new();

        let mut writes = BVec::with_capacity_in(3, bump);

        let mut query = world.query::<(&Renderable, &Isometry3)>();
        for (entity, (renderable, iso)) in query.iter() {
            if let Some(blas) = input.blases.get(&renderable.mesh) {
                let blas_address =
                    ctx.get_acceleration_structure_device_address(blas);

                let m = match renderable.transform {
                    Some(t) => iso.into_homogeneous_matrix() * t,
                    None => iso.into_homogeneous_matrix(),
                };

                acc_instances.push(
                    AccelerationStructureInstance::new(blas_address)
                        .with_transform(m.into()),
                );

                let (mesh_index, new) =
                    self.meshes.index(renderable.mesh.clone());
                if new {
                    let binding = &renderable.mesh.bindings()[0];

                    assert_eq!(
                        binding.layout,
                        PositionNormalTangent3dUV::layout()
                    );

                    let indices = renderable.mesh.indices().unwrap();
                    let indices_buffer = indices.buffer.clone();
                    let indices_offset = indices.offset;
                    let indices_size: u64 = indices.index_type.size() as u64
                        * renderable.mesh.count() as u64;

                    let vertices_buffer = binding.buffer.clone();
                    let vertices_offset = binding.offset;
                    let vertices_size: u64 = binding.layout.stride as u64
                        * renderable.mesh.vertex_count() as u64;

                    assert_eq!(indices_offset & 15, 0);
                    assert_eq!(vertices_offset & 15, 0);

                    let indices_descriptors =
                        Descriptors::StorageBuffer(bump.alloc([(
                            indices_buffer,
                            indices_offset,
                            indices_size,
                        )]));

                    let vertices_descriptors =
                        Descriptors::StorageBuffer(bump.alloc([(
                            vertices_buffer,
                            vertices_offset,
                            vertices_size,
                        )]));

                    writes.push(WriteDescriptorSet {
                        set: &self.set,
                        binding: 2,
                        element: mesh_index,
                        descriptors: indices_descriptors,
                    });

                    writes.push(WriteDescriptorSet {
                        set: &self.set,
                        binding: 3,
                        element: mesh_index,
                        descriptors: vertices_descriptors,
                    });
                }

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
                    transform: iso.into_homogeneous_matrix(),
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
                });
            } else {
                tracing::error!("Missing BLAS for mesh @ {:?}", entity);
            }
        }

        ctx.update_descriptor_sets(&writes, &[]);
        drop(writes);

        ensure!(
            instances.len() <= MAX_INSTANCE_COUNT.into(),
            "Too many instances"
        );

        ensure!(u32::try_from(instances.len()).is_ok(), "Too many instances");

        let mut encoder = ctx.queue.create_encoder()?;

        // Sync BLAS and TLAS builds.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
        );

        ctx.write_memory(
            &self.globals_and_instances,
            acc_instances_offset(fid),
            &acc_instances,
        );

        ctx.write_memory(
            &self.globals_and_instances,
            instances_offset(fid),
            &instances,
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
                        .offset(acc_instances_offset(fid)),
                    primitive_count: instances.len() as u32,
                },
            ]),
            scratch: ctx.get_buffer_device_address(&self.scratch).unwrap(),
        }]);

        encoder.build_acceleration_structure(infos);

        let dirlight = world
            .query::<&DirectionalLight>()
            .iter()
            .next()
            .map(|(_, dl)| GlobalsDirLight {
                rad: Vec3::from(dl.radiance),
                dir: dl.direction,
                _pad0: 0.0,
                _pad1: 0.0,
            })
            .unwrap_or(GlobalsDirLight {
                rad: Vec3::zero(),
                dir: Vec3::zero(),
                _pad0: 0.0,
                _pad1: 0.0,
            });

        let skylight = world
            .query::<&SkyLight>()
            .iter()
            .next()
            .map(|(_, sl)| Vec3::from(sl.radiance))
            .unwrap_or_default();

        let globals = Globals {
            camera: GlobalsCamera {
                view: input.camera_transform,
                iview: input.camera_transform.inversed(),
                proj: input.camera_projection,
                iproj: input.camera_projection.inversed(),
            },
            dirlight,
            skylight,
            _pad: 0.0,
            // frame: frame as u32,
            frame: 0,
        };

        ctx.write_memory(
            &self.globals_and_instances,
            globals_offset(fid),
            std::slice::from_ref(&globals),
        );

        encoder.bind_ray_tracing_pipeline(&self.pipeline);

        encoder.bind_ray_tracing_descriptor_sets(
            &self.pipeline_layout,
            0,
            bump.alloc([
                self.set.clone(),
                self.per_frame_sets[fid as usize].clone(),
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

        ctx.queue.submit(wait, encoder.finish(), signal, fence);

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
    view: Mat4,
    proj: Mat4,
    iview: Mat4,
    iproj: Mat4,
}

unsafe impl Zeroable for GlobalsCamera {}
unsafe impl Pod for GlobalsCamera {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GlobalsDirLight {
    dir: Vec3,
    _pad0: f32,
    rad: Vec3,
    _pad1: f32,
}

unsafe impl Zeroable for GlobalsDirLight {}
unsafe impl Pod for GlobalsDirLight {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Globals {
    camera: GlobalsCamera,
    dirlight: GlobalsDirLight,
    skylight: Vec3,
    _pad: f32,
    frame: u32,
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
    size_of::<[ShaderInstance; 1024]>() as u64
}

fn instances_offset(frame: u32) -> u64 {
    align_up(255, globals_end(1)).unwrap()
        + u64::from(frame) * align_up(255, instances_size()).unwrap()
}

fn instances_end(frame: u32) -> u64 {
    instances_offset(frame) + instances_size()
}

const fn acc_instances_size() -> u64 {
    size_of::<[AccelerationStructure; 1024]>() as u64
}

fn acc_instances_offset(frame: u32) -> u64 {
    align_up(255, instances_end(1)).unwrap()
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

mod mesh;
mod vertex;

pub use self::{mesh::*, vertex::*};

use {
    crate::{camera::Camera, clocks::ClockIndex, light::DirectionalLight},
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    eyre::{bail, ensure, eyre, WrapErr as _},
    goods::Asset,
    hecs::World,
    illume::*,
    std::{
        collections::hash_map::{Entry, HashMap},
        convert::TryFrom as _,
        mem::{align_of, size_of, size_of_val},
    },
    ultraviolet::{Mat4, Vec3},
    winit::window::Window,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse renderable metadata {source}")]
    Metadata {
        #[from]
        source: ron::de::Error,
    },
}

pub struct Renderable {
    index: u16,
}

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    swapchain: Swapchain,

    dset_layout: DescriptorSetLayout,
    rt_pipeline_layout: PipelineLayout,
    prepass_pipeline: RayTracingPipeline,

    tlas: AccelerationStructure,
    instances: Vec<AccelerationStructureInstance>,
    tlas_scratch: Buffer,

    buffer: Buffer,
    per_frame: [Frame; 2],
    shader_binding_table: ShaderBindingTable,

    blue_noise_buffer_64x64x64: Buffer,

    meshes: HashMap<Mesh, u16>,
    blases: Vec<(Mesh, AccelerationStructure)>,
    scene_instances: Vec<SceneInstance>,

    normals: (Image, ImageView),
    frame: u32,
}

struct Frame {
    set: DescriptorSet,
    fence: Fence,
}

impl Renderer {
    pub fn new(window: &Window) -> Result<Self, Report> {
        let graphics = enumerate_graphis()
            .next()
            .ok_or_else(|| eyre!("No graphics found"))?;

        tracing::debug!("{:?}", graphics);

        // Create surface for window.
        let mut surface = graphics.create_surface(window)?;

        // Find suitable device.
        let mut devices = graphics.devices()?;

        let (physical, surface_caps) = loop {
            if let Some(d) = devices.next() {
                if let Some(caps) = d.surface_capabilities(&surface)? {
                    break (d, caps);
                }
            } else {
                bail!("No devices found");
            }
        };

        tracing::debug!("{:?}", physical);
        tracing::debug!("{:?}", surface_caps);

        let device_info = physical.info();
        tracing::debug!("{:?}", device_info);

        // Initialize device.
        let (device, queue) = physical.create_device(
            &[
                Feature::RayTracing,
                Feature::BufferDeviceAddress,
                Feature::SurfacePresentation,
                Feature::RuntimeDescriptorArray,
                Feature::ScalarBlockLayout,
                Feature::DescriptorBindingUpdateUnusedWhilePending,
                Feature::DescriptorBindingPartiallyBound,
            ],
            SingleQueueQuery::GENERAL,
        )?;

        tracing::debug!("{:?}", device);

        // Configure swapchain.
        let mut swapchain = device.create_swapchain(&mut surface)?;

        tracing::debug!("{:?}", swapchain);

        let format = *surface_caps
            .formats
            .iter()
            .filter(|format| {
                use FormatDescription as FD;

                match format.description() {
                    FD::RGB(_) | FD::RGBA(_) | FD::BGR(_) | FD::BGRA(_) => true,
                    _ => false,
                }
            })
            .max_by_key(|format| match format.color_type() {
                Some(FormatType::Srgb) => 1,
                _ => 0,
            })
            .ok_or_else(|| eyre!("No surface format found"))?;

        tracing::debug!("Surface format: {:?}", format);

        swapchain.configure(
            ImageUsage::TRANSFER_DST,
            format,
            PresentMode::Fifo,
        )?;

        tracing::trace!("Swapchain configured");

        // Create pipeline.
        let dset_layout =
            device.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::UPDATE_AFTER_BIND_POOL,
                bindings: vec![
                    // TLAS.
                    DescriptorSetLayoutBinding {
                        binding: TLAS_BINDING,
                        ty: DescriptorType::AccelerationStructure,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Globals
                    DescriptorSetLayoutBinding {
                        binding: GLOBAL_BINDING,
                        ty: DescriptorType::UniformBuffer,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    // Scene
                    DescriptorSetLayoutBinding {
                        binding: SCENE_BINDING,
                        ty: DescriptorType::StorageBuffer,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN
                            | ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                    // Vertex input.
                    DescriptorSetLayoutBinding {
                        binding: VERTICES_BINDING,
                        ty: DescriptorType::StorageBuffer,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    // Indices
                    DescriptorSetLayoutBinding {
                        binding: INDICES_BINDING,
                        ty: DescriptorType::StorageBuffer,
                        count: MAX_INSTANCE_COUNT.into(),
                        stages: ShaderStageFlags::CLOSEST_HIT,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    },
                    // G-Buffer
                    // Normal
                    DescriptorSetLayoutBinding {
                        binding: NORMALS_BINDING,
                        ty: DescriptorType::StorageImage,
                        count: 1,
                        stages: ShaderStageFlags::RAYGEN,
                        flags: DescriptorBindingFlags::empty(),
                    },
                ],
            })?;

        let rt_pipeline_layout =
            device.create_pipeline_layout(PipelineLayoutInfo {
                sets: vec![dset_layout.clone()],
            })?;

        let prepass_rgen = RaygenShader::with_main(
            device.create_shader_module(
                Spirv::new(include_bytes!("shaders/prepass.rgen.spv").to_vec())
                    .into(),
            )?,
        );

        let prepass_rmiss = MissShader::with_main(
            device.create_shader_module(
                Spirv::new(
                    include_bytes!("shaders/prepass.rmiss.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let prepass_rchit = ClosestHitShader::with_main(
            device.create_shader_module(
                Spirv::new(
                    include_bytes!("shaders/prepass.rchit.spv").to_vec(),
                )
                .into(),
            )?,
        );

        let shadow_rmiss = MissShader::with_main(
            device.create_shader_module(
                Spirv::new(include_bytes!("shaders/shadow.rmiss.spv").to_vec())
                    .into(),
            )?,
        );

        let prepass_pipeline =
            device.create_ray_tracing_pipeline(RayTracingPipelineInfo {
                shaders: vec![
                    prepass_rgen.into(),
                    prepass_rmiss.into(),
                    prepass_rchit.into(),
                    shadow_rmiss.into(),
                ],
                groups: vec![
                    RayTracingShaderGroupInfo::Raygen { raygen: 0 },
                    RayTracingShaderGroupInfo::Miss { miss: 1 },
                    RayTracingShaderGroupInfo::Miss { miss: 3 },
                    RayTracingShaderGroupInfo::Triangles {
                        any_hit: None,
                        closest_hit: Some(2),
                    },
                ],
                max_recursion_depth: 10,
                layout: rt_pipeline_layout.clone(),
            })?;

        let shader_binding_table = device
            .create_ray_tracing_shader_binding_table(
                &prepass_pipeline,
                ShaderBindingTableInfo {
                    raygen: Some(0),
                    miss: &[1],
                    hit: &[3],
                    callable: &[],
                },
            )?;

        tracing::trace!("RT pipeline created");

        // Creating TLAS.
        let tlas = device.create_acceleration_structure(
            AccelerationStructureInfo {
                level: AccelerationStructureLevel::Top,
                flags: AccelerationStructureFlags::empty(),
                geometries: vec![
                    AccelerationStructureGeometryInfo::Instances {
                        max_primitive_count: MAX_INSTANCE_COUNT.into(),
                    },
                ],
            },
        )?;

        tracing::trace!("TLAS created");
        // Allocate scratch memory for TLAS building.
        let tlas_scratch = device
            .allocate_acceleration_structure_build_scratch(&tlas, false)?;

        tracing::trace!("TLAS scratch allocated");

        let buffer = device.create_buffer(BufferInfo {
            align: buffer_align(),
            size: buffer_size(),
            usage: BufferUsage::UNIFORM
                | BufferUsage::STORAGE
                | BufferUsage::RAY_TRACING
                | BufferUsage::SHADER_DEVICE_ADDRESS,
            memory: MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::FAST_DEVICE_ACCESS,
        })?;

        tracing::trace!("Globals and instances buffer created");

        let windows_inner_size = window.inner_size();
        let surface_extent = Extent2d {
            width: windows_inner_size.width,
            height: windows_inner_size.height,
        };

        // Image matching surface extent.
        let normals_image = device.create_image(ImageInfo {
            extent: surface_extent.into(),
            format: Format::RGBA32Sfloat,
            levels: 1,
            layers: 1,
            samples: Samples::Samples1,
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
        })?;

        // View for whole image
        let normals_view = device.create_image_view(ImageViewInfo {
            image: normals_image.clone(),
            view_kind: ImageViewKind::D2,
            subresource: ImageSubresourceRange::whole(normals_image.info()),
        })?;

        tracing::trace!("Normals image created");

        let set0 = device.create_descriptor_set(DescriptorSetInfo {
            layout: dset_layout.clone(),
        })?;

        let set1 = device.create_descriptor_set(DescriptorSetInfo {
            layout: dset_layout.clone(),
        })?;

        tracing::trace!("Descriptor sets created");

        device.update_descriptor_sets(
            &[
                WriteDescriptorSet {
                    set: &set0,
                    binding: TLAS_BINDING,
                    element: 0,
                    descriptors: Descriptors::AccelerationStructure(
                        std::slice::from_ref(&tlas),
                    ),
                },
                WriteDescriptorSet {
                    set: &set1,
                    binding: TLAS_BINDING,
                    element: 0,
                    descriptors: Descriptors::AccelerationStructure(
                        std::slice::from_ref(&tlas),
                    ),
                },
                WriteDescriptorSet {
                    set: &set0,
                    binding: GLOBAL_BINDING,
                    element: 0,
                    descriptors: Descriptors::UniformBuffer(&[(
                        buffer.clone(),
                        globals_offset(0),
                        globals_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &set1,
                    binding: GLOBAL_BINDING,
                    element: 0,
                    descriptors: Descriptors::UniformBuffer(&[(
                        buffer.clone(),
                        globals_offset(1),
                        globals_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &set0,
                    binding: SCENE_BINDING,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        buffer.clone(),
                        scene_instances_offset(0),
                        scene_instances_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &set1,
                    binding: SCENE_BINDING,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&[(
                        buffer.clone(),
                        scene_instances_offset(1),
                        scene_instances_size(),
                    )]),
                },
                WriteDescriptorSet {
                    set: &set0,
                    binding: NORMALS_BINDING,
                    element: 0,
                    descriptors: Descriptors::StorageImage(&[(
                        normals_view.clone(),
                        Layout::General,
                    )]),
                },
                WriteDescriptorSet {
                    set: &set1,
                    binding: NORMALS_BINDING,
                    element: 0,
                    descriptors: Descriptors::StorageImage(&[(
                        normals_view.clone(),
                        Layout::General,
                    )]),
                },
            ],
            &[],
        );

        tracing::trace!("Descriptor sets written");

        let blue_noise_buffer_64x64x64 = load_blue_noise_64x64x64(&device)?;

        Ok(Renderer {
            rt_pipeline_layout,
            prepass_pipeline,

            meshes: HashMap::new(),
            blases: Vec::new(),
            scene_instances: Vec::new(),

            tlas,
            instances: Vec::new(),
            tlas_scratch,

            buffer,
            per_frame: [
                Frame {
                    set: set0,
                    fence: device.create_fence()?,
                },
                Frame {
                    set: set1,
                    fence: device.create_fence()?,
                },
            ],
            shader_binding_table,
            blue_noise_buffer_64x64x64,

            normals: (normals_image, normals_view),
            frame: 0,

            device,
            queue,
            dset_layout,

            swapchain,
        })
    }

    pub fn draw(
        &mut self,
        world: &mut World,
        clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<(), Report> {
        tracing::debug!("Rendering next frame");

        let mut cameras = world.query::<(&Camera, &Mat4)>();
        let camera = if let Some((_, camera)) = cameras.iter().next() {
            camera
        } else {
            tracing::warn!("No camera found");
            return Ok(());
        };
        let cam_tr = *camera.1;
        let cam_proj = camera.0.projection();
        drop(cameras);

        let mut dirlights = world.query::<&DirectionalLight>();
        let dirlight = dirlights.iter().next().map(|(_, l)| l);
        let dirlight = GlobalsDirLight {
            dir: dirlight.map(|l| l.direction).unwrap_or(-Vec3::unit_z()),
            _pad0: 0.0,
            rad: dirlight
                .map(|l| {
                    let [r, g, b] = l.radiance;
                    Vec3::new(r, g, b)
                })
                .unwrap_or(Vec3::zero()),
            _pad1: 0.0,
        };

        drop(dirlights);

        let fid = self.frame % 2;
        let mut encoder = self.queue.create_encoder()?;

        // Update descriptors.
        let mut writes = BVec::with_capacity_in(3, bump);

        // Create BLASes for new meshes.
        let mut new_entities = BVec::with_capacity_in(32, bump);
        for (entity, mesh) in world
            .query::<&Mesh>()
            .with::<Mat4>()
            .without::<Renderable>()
            .iter()
        {
            let index = match self.meshes.entry(mesh.clone()) {
                Entry::Vacant(entry) => {
                    let index = u16::try_from(self.blases.len())
                        .wrap_err("Too many meshes")?;
                    self.blases.push((
                        mesh.clone(),
                        mesh.build_triangles_blas(
                            &mut encoder,
                            &self.device,
                            bump,
                        )?,
                    ));

                    let binding = &mesh.bindings()[0];
                    let buffer = binding.buffer.clone();
                    let offset = binding.offset
                        + binding.layout.locations.as_ref()[0].offset as u64;
                    let size: u64 = binding.layout.stride as u64
                        * mesh.vertex_count() as u64;

                    let descriptors = Descriptors::StorageBuffer(
                        bump.alloc([(buffer, offset, size)]),
                    );

                    writes.push(WriteDescriptorSet {
                        set: &self.per_frame[0].set,
                        binding: VERTICES_BINDING,
                        element: index.into(),
                        descriptors,
                    });
                    writes.push(WriteDescriptorSet {
                        set: &self.per_frame[1].set,
                        binding: VERTICES_BINDING,
                        element: index.into(),
                        descriptors,
                    });

                    let indices = &mesh.indices().unwrap();
                    let buffer = indices.buffer.clone();
                    let offset = indices.offset;
                    let size: u64 =
                        indices.index_type.size() as u64 * mesh.count() as u64;

                    let descriptors = Descriptors::StorageBuffer(
                        bump.alloc([(buffer, offset, size)]),
                    );

                    writes.push(WriteDescriptorSet {
                        set: &self.per_frame[0].set,
                        binding: INDICES_BINDING,
                        element: index.into(),
                        descriptors,
                    });
                    writes.push(WriteDescriptorSet {
                        set: &self.per_frame[1].set,
                        binding: INDICES_BINDING,
                        element: index.into(),
                        descriptors,
                    });

                    *entry.insert(index)
                }
                Entry::Occupied(entry) => *entry.get(),
            };

            new_entities.push((entity, index));
        }

        self.device.update_descriptor_sets(&writes, &[]);
        drop(writes);

        // Sync BLAS and TLAS builds.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
        );

        // Insert them to the entities.
        for (entity, index) in new_entities {
            world.insert_one(entity, Renderable { index }).unwrap();
        }

        // https://microsoft.github.io/DirectX-Specs/d3d/Raytracing.html#general-tips-for-building-acceleration-structures
        //
        // > Rebuild top-level acceleration structure every frame
        //   Only updating instead of rebuilding is rarely the right thing to
        // do.   Rebuilds for a few thousand instances are very fast,
        //   and having a good quality top-level acceleration structure can have
        // a significant payoff   (bad quality has a higher cost further
        // up in the tree).
        self.instances.clear();
        self.scene_instances.clear();
        for (_, (renderable, &transform)) in
            world.query::<(&Renderable, &Mat4)>().iter()
        {
            let blas = &self.blases[usize::from(renderable.index)].1;
            let blas_address =
                self.device.get_acceleration_structure_device_address(blas);

            self.instances.push(
                AccelerationStructureInstance::new(blas_address)
                    .with_transform(transform.into()),
            );
            self.scene_instances.push(SceneInstance {
                transform,
                mesh: renderable.index.into(),
            });
        }

        ensure!(
            self.instances.len() <= MAX_INSTANCE_COUNT.into(),
            "Too many instances"
        );

        ensure!(
            u32::try_from(self.instances.len()).is_ok(),
            "Too many instances"
        );

        if self.frame > 1 {
            let fence = &self.per_frame[(fid) as usize].fence;
            self.device.wait_fences(&[fence], true);
            self.device.reset_fences(&[fence])
        }

        let frame = self.swapchain.acquire_image()?.unwrap();

        let frame_image = &frame.info().image;

        self.device.write_memory(
            &self.buffer,
            scene_instances_offset(fid),
            &self.scene_instances,
        );

        self.device.write_memory(
            &self.buffer,
            acc_instances_offset(fid),
            &self.instances,
        );

        let infos = bump.alloc([AccelerationStructureBuildGeometryInfo {
            src: None,
            dst: self.tlas.clone(),
            geometries: bump.alloc([
                AccelerationStructureGeometry::Instances {
                    flags: GeometryFlags::OPAQUE,
                    data: self
                        .device
                        .get_buffer_device_address(&self.buffer)
                        .unwrap()
                        .offset(acc_instances_offset(fid)),
                    primitive_count: self.instances.len() as u32,
                },
            ]),
            scratch: self
                .device
                .get_buffer_device_address(&self.tlas_scratch)
                .unwrap(),
        }]);

        encoder.build_acceleration_structure(infos);

        // Sync TLAS build with ray-tracing shader where it will be used.
        encoder.pipeline_barrier(
            PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD,
            PipelineStageFlags::RAY_TRACING_SHADER,
        );

        let globals = Globals {
            camera: GlobalsCamera {
                view: cam_tr.inversed(),
                iview: cam_tr,
                proj: cam_proj,
                iproj: cam_proj.inversed(),
            },
            dirlight,
            seconds: (clock.step - clock.start).as_secs_f32(),
            frame: self.frame,
        };

        self.device.write_memory(
            &self.buffer,
            globals_offset(fid),
            std::slice::from_ref(&globals),
        );

        encoder.bind_ray_tracing_pipeline(&self.prepass_pipeline);

        encoder.bind_ray_tracing_descriptor_sets(
            &self.rt_pipeline_layout,
            0,
            std::slice::from_ref(&self.per_frame[fid as usize].set),
            &[],
        );

        // Sync storage image access from last frame blit operation to this
        // frame writes in raygen shaders.
        let images = [ImageLayoutTransition::initialize_whole(
            &self.normals.0,
            Layout::General,
        )
        .into()];

        encoder.image_barriers(
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::RAY_TRACING_SHADER,
            &images,
        );

        let rendering_extent = frame_image.info().extent.into_3d();

        // Perform ray-trace operation.
        encoder.trace_rays(&self.shader_binding_table, rendering_extent);

        // Sync storage image access from writes in raygen shader to blit
        // operation. And swapchain image from presentation to transfer
        let images = [
            ImageLayoutTransition::transition_whole(
                &self.normals.0,
                Layout::General..Layout::TransferSrcOptimal,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &frame_image,
                Layout::TransferDstOptimal,
            )
            .into(),
        ];

        encoder.image_barriers(
            PipelineStageFlags::RAY_TRACING_SHADER
                | PipelineStageFlags::TRANSFER,
            PipelineStageFlags::TRANSFER,
            &images,
        );

        // Blit ray-tracing result image to the frame.
        let blit = [ImageBlit {
            src_subresource: ImageSubresourceLayers::all_layers(
                self.normals.0.info(),
                0,
            ),
            src_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(rendering_extent)?,
            ],
            dst_subresource: ImageSubresourceLayers::all_layers(
                frame_image.info(),
                0,
            ),
            dst_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(rendering_extent)?,
            ],
        }];

        encoder.blit_image(
            &self.normals.0,
            Layout::TransferSrcOptimal,
            &frame_image,
            Layout::TransferDstOptimal,
            &blit,
            Filter::Linear,
        );

        // Sync swapchain image from transfer to presentation.
        let images = [ImageLayoutTransition::transition_whole(
            &frame_image,
            Layout::TransferDstOptimal..Layout::Present,
        )
        .into()];

        encoder.image_barriers(
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::BOTTOM_OF_PIPE,
            &images,
        );

        // Submit execution.
        self.queue.submit(
            &[(PipelineStageFlags::all(), frame.info().wait.clone())],
            encoder.finish(),
            &[frame.info().signal.clone()],
            Some(&self.per_frame[fid as usize].fence),
        );

        // Present the frame.
        self.queue.present(frame);

        self.frame += 1;

        Ok(())
    }
}

const MAX_INSTANCE_COUNT: u16 = 1024;

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
    seconds: f32,
    frame: u32,
}

unsafe impl Zeroable for Globals {}
unsafe impl Pod for Globals {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SceneInstance {
    transform: Mat4,
    mesh: u32,
}

unsafe impl Zeroable for SceneInstance {}
unsafe impl Pod for SceneInstance {}

const fn globals_size() -> u64 {
    size_of::<Globals>() as u64
}

fn globals_offset(frame: u32) -> u64 {
    u64::from(frame) * align_up(255, globals_size()).unwrap()
}

fn globals_end(frame: u32) -> u64 {
    globals_offset(frame) + globals_size()
}

const fn scene_instances_size() -> u64 {
    size_of::<[SceneInstance; 1024]>() as u64
}

fn scene_instances_offset(frame: u32) -> u64 {
    align_up(255, globals_end(1)).unwrap()
        + u64::from(frame) * align_up(255, scene_instances_size()).unwrap()
}

fn scene_instances_end(frame: u32) -> u64 {
    scene_instances_offset(frame) + scene_instances_size()
}

const fn acc_instances_size() -> u64 {
    size_of::<[AccelerationStructure; 1024]>() as u64
}

fn acc_instances_offset(frame: u32) -> u64 {
    align_up(255, scene_instances_end(1)).unwrap()
        + u64::from(frame) * align_up(255, acc_instances_size()).unwrap()
}

fn acc_instances_end(frame: u32) -> u64 {
    acc_instances_offset(frame) + acc_instances_size()
}

const fn buffer_align() -> u64 {
    255
}

fn buffer_size() -> u64 {
    acc_instances_end(1)
}

const TLAS_BINDING: u32 = 0;
const GLOBAL_BINDING: u32 = 1;
const SCENE_BINDING: u32 = 2;
const VERTICES_BINDING: u32 = 3;
const INDICES_BINDING: u32 = 4;
const NORMALS_BINDING: u32 = 5;

fn load_blue_noise_64x64x64(device: &Device) -> Result<Buffer, OutOfMemory> {
    use image::{load_from_memory_with_format, ImageFormat};

    let images = [
        &include_bytes!("../blue_noise/HDR_RGBA_0.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_1.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_2.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_3.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_4.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_5.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_6.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_7.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_8.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_9.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_10.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_11.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_12.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_13.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_14.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_15.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_16.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_17.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_18.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_19.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_20.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_21.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_22.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_23.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_24.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_25.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_26.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_27.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_28.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_29.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_30.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_31.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_32.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_33.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_34.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_35.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_36.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_37.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_38.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_39.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_40.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_41.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_42.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_43.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_44.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_45.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_46.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_47.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_48.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_49.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_50.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_51.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_52.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_53.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_54.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_55.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_56.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_57.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_58.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_59.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_60.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_61.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_62.png")[..],
        &include_bytes!("../blue_noise/HDR_RGBA_63.png")[..],
    ];

    let mut bytes = Vec::new();

    for &image in &images[..] {
        let image = load_from_memory_with_format(image, ImageFormat::Png)
            .unwrap()
            .to_rgba();
        bytes.append(&mut image.into_raw());
    }

    device.create_buffer_static(
        BufferInfo {
            align: 255,
            size: bytes.len() as _,
            usage: BufferUsage::STORAGE,
            memory: MemoryUsageFlags::UPLOAD,
        },
        &bytes,
    )
}

/// Enumerate graphics backends.
pub fn enumerate_graphis() -> impl Iterator<Item = Graphics> {
    #[allow(unused_mut)]
    let mut fns = Vec::new();

    #[cfg(feature = "vulkan")]
    {
        fns.push(
            illume_erupt::EruptGraphics::try_init as fn() -> Option<Graphics>,
        );
    }

    #[cfg(feature = "webgl")]
    {
        fns.push(
            illume_webgl::WebGlGraphics::try_init as fn() -> Option<Graphics>,
        );
    }

    fns.into_iter()
        .filter_map(|try_init: fn() -> Option<Graphics>| try_init())
}

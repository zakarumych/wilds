mod material;
mod mesh;
mod pass;
mod vertex;

pub use {
    self::{material::*, mesh::*, vertex::*},
    illume::*,
};

use {
    self::pass::*,
    crate::{camera::Camera, clocks::ClockIndex, scene::Global3},
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::Pod,
    color_eyre::Report,
    eyre::eyre,
    hecs::World,
    nalgebra as na,
    std::{
        collections::hash_map::{Entry, HashMap},
        convert::TryFrom as _,
        ops::{Deref, DerefMut},
    },
    type_map::TypeMap,
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

#[derive(Clone, Debug)]
pub struct Renderable {
    pub mesh: Mesh,
    pub material: Material,
    // pub transform: Option<na::Matrix4<f32>>,
}

pub struct Context {
    pub device: Device,
    pub queue: Queue,
    buffer_uploads: Vec<BufferUpload>,
    image_uploads: Vec<ImageUpload>,
}

impl Context {
    pub fn upload_buffer<T>(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        data: &[T],
    ) -> Result<(), MappingError>
    where
        T: Pod,
    {
        if buffer.info().memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.write_memory(buffer, offset, data)?;
            Ok(())
        } else {
            let staging = self.device.create_buffer_static(
                BufferInfo {
                    align: 15,
                    size: u64::try_from(data.len()).map_err(|_| OutOfMemory)?,
                    usage: BufferUsage::TRANSFER_SRC,
                    memory: MemoryUsageFlags::UPLOAD,
                },
                data,
            )?;

            self.buffer_uploads.push(BufferUpload {
                staging,
                buffer: buffer.clone(),
                offset,
            });

            Ok(())
        }
    }

    pub fn upload_image<T>(
        &mut self,
        image: &Image,
        layout: Option<Layout>,
        row_length: u32,
        image_height: u32,
        subresource: ImageSubresourceLayers,
        offset: Offset3d,
        extent: Extent3d,
        data: &[T],
    ) -> Result<(), OutOfMemory>
    where
        T: Pod,
    {
        let staging = self.device.create_buffer_static(
            BufferInfo {
                align: 15,
                size: u64::try_from(data.len()).map_err(|_| OutOfMemory)?,
                usage: BufferUsage::TRANSFER_SRC,
                memory: MemoryUsageFlags::UPLOAD,
            },
            data,
        )?;

        self.image_uploads.push(ImageUpload {
            staging,
            image: image.clone(),
            layout,
            row_length,
            image_height,
            subresource,
            offset,
            extent,
        });

        Ok(())
    }

    pub fn create_buffer_static<T>(
        &mut self,
        mut info: BufferInfo,
        data: &[T],
    ) -> Result<Buffer, OutOfMemory>
    where
        T: Pod,
    {
        assert!(arith_ge(info.size, data.len()));
        if info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.create_buffer_static(info, data)
        } else {
            info.usage |= BufferUsage::TRANSFER_DST;
            let buffer = self.device.create_buffer(info)?;
            match self.upload_buffer(&buffer, 0, data) {
                Ok(()) => {}
                Err(MappingError::OutOfMemory { .. }) => {
                    return Err(OutOfMemory)
                }
                _ => unreachable!(),
            }
            Ok(buffer)
        }
    }

    pub fn create_image_static<T>(
        &mut self,
        mut info: ImageInfo,
        row_length: u32,
        image_height: u32,
        data: &[T],
    ) -> Result<Image, CreateImageError>
    where
        T: Pod,
    {
        info.usage |= ImageUsage::TRANSFER_DST;
        let subresource = ImageSubresourceLayers::all_layers(&info, 0);
        let image = self.device.create_image(info)?;
        self.upload_image(
            &image,
            None,
            row_length,
            image_height,
            subresource,
            Offset3d::ZERO,
            info.extent.into_3d(),
            data,
        )?;
        Ok(image)
    }

    fn flush_uploads(&mut self, bump: &Bump) -> Result<(), Report> {
        if self.buffer_uploads.is_empty() && self.image_uploads.is_empty() {
            return Ok(());
        }

        let mut encoder = self.queue.create_encoder()?;

        if !self.buffer_uploads.is_empty() {
            tracing::debug!("Uploading buffers");

            for upload in &self.buffer_uploads {
                encoder.copy_buffer(
                    &upload.staging,
                    &upload.buffer,
                    bump.alloc([BufferCopy {
                        src_offset: 0,
                        dst_offset: upload.offset,
                        size: upload.staging.info().size,
                    }]),
                )
            }
        }

        if !self.image_uploads.is_empty() {
            tracing::debug!("Uploading images");

            let mut images =
                BVec::with_capacity_in(self.image_uploads.len(), bump);

            for upload in &self.image_uploads {
                let switch_layout = match upload.layout {
                    Some(Layout::General)
                    | Some(Layout::TransferDstOptimal) => false,
                    _ => true,
                };

                if switch_layout {
                    images.push(ImageMemoryBarrier {
                        image: bump.alloc(upload.image.clone()),
                        old_layout: None,
                        new_layout: Layout::TransferDstOptimal,
                        family_transfer: None,
                        subresource: ImageSubresourceRange::whole(
                            upload.image.info(),
                        ),
                    });
                }
            }

            let images_len = images.len();

            encoder.image_barriers(
                PipelineStageFlags::TOP_OF_PIPE,
                PipelineStageFlags::TRANSFER,
                images.into_bump_slice(),
            );

            for upload in &self.image_uploads {
                encoder.copy_buffer_to_image(
                    &upload.staging,
                    &upload.image,
                    if upload.layout == Some(Layout::General) {
                        Layout::General
                    } else {
                        Layout::TransferDstOptimal
                    },
                    bump.alloc([BufferImageCopy {
                        buffer_offset: 0,
                        buffer_row_length: upload.row_length,
                        buffer_image_height: upload.image_height,
                        image_subresource: upload.subresource,
                        image_offset: upload.offset,
                        image_extent: upload.extent,
                    }]),
                )
            }

            let mut images = BVec::with_capacity_in(images_len, bump);

            for upload in &self.image_uploads {
                let switch_layout = match upload.layout {
                    Some(Layout::General)
                    | Some(Layout::TransferDstOptimal) => false,
                    _ => true,
                };

                if switch_layout {
                    images.push(ImageMemoryBarrier {
                        image: bump.alloc(upload.image.clone()),
                        old_layout: Some(Layout::TransferDstOptimal),
                        new_layout: upload.layout.unwrap_or(Layout::General),
                        family_transfer: None,
                        subresource: ImageSubresourceRange::whole(
                            upload.image.info(),
                        ),
                    });
                }
            }

            encoder.image_barriers(
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::TOP_OF_PIPE,
                images.into_bump_slice(),
            );
        }

        self.queue.submit_no_semaphores(encoder.finish(), None);

        self.buffer_uploads.clear();
        self.image_uploads.clear();
        Ok(())
    }
}

impl Deref for Context {
    type Target = Device;

    fn deref(&self) -> &Device {
        &self.device
    }
}

pub struct RenderConstants {
    pub filter_enabled: bool,
}

impl RenderConstants {
    pub const fn new() -> Self {
        RenderConstants {
            filter_enabled: true,
        }
    }
}

pub struct Renderer {
    context: Context,

    fences: [Fence; 2],

    frame: u64,

    blases: HashMap<Mesh, AccelerationStructure>,

    swapchain: Swapchain,
    pose: PosePass,
    rt_prepass: RtPrepass,
    diffuse_filter: ATrousFilter,
    direct_filter: ATrousFilter,
    combine: CombinePass,
}

impl Deref for Renderer {
    type Target = Context;

    fn deref(&self) -> &Context {
        &self.context
    }
}

impl DerefMut for Renderer {
    fn deref_mut(&mut self) -> &mut Context {
        &mut self.context
    }
}

impl Renderer {
    pub fn new(window: &Window) -> Result<Self, Report> {
        let graphics = Graphics::get_or_init()?;

        tracing::debug!("{:?}", graphics);

        // Create surface for window.
        let mut surface = graphics.create_surface(window)?;

        let devices = graphics.devices()?;

        // Find suitable device.
        let (physical, surface_caps) = devices
            .into_iter()
            .filter_map(|d| {
                let caps = d.surface_capabilities(&surface).ok().flatten()?;
                Some((d, caps))
            })
            .next()
            .ok_or_else(|| eyre!("No devices found"))?;

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

        let mut context = Context {
            device,
            queue,
            buffer_uploads: Vec::new(),
            image_uploads: Vec::new(),
        };

        let size = window.inner_size();
        let window_extent = Extent2d {
            width: size.width,
            height: size.height,
        };

        let mut swapchain = context.create_swapchain(&mut surface)?;
        swapchain.configure(
            ImageUsage::COLOR_ATTACHMENT,
            format,
            PresentMode::Fifo,
        )?;

        let pose = PosePass::new(&mut context)?;
        let rt_prepass = RtPrepass::new(window_extent, &mut context)?;

        let combine = CombinePass::new(&mut context)?;
        let diffuse_filter = ATrousFilter::new(&mut context)?;
        let direct_filter = ATrousFilter::new(&mut context)?;

        Ok(Renderer {
            fences: [context.create_fence()?, context.create_fence()?],
            frame: 0,
            blases: HashMap::new(),
            swapchain,
            pose,
            rt_prepass,
            diffuse_filter,
            direct_filter,
            combine,
            context,
        })
    }

    pub fn draw(
        &mut self,
        world: &mut World,
        resources: &TypeMap,
        _clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<(), Report> {
        const DEFAULT_CONSTANTS: RenderConstants = RenderConstants::new();

        let constants = resources
            .get::<RenderConstants>()
            .unwrap_or(&DEFAULT_CONSTANTS);

        self.context.flush_uploads(bump)?;

        tracing::debug!("Rendering next frame");

        let mut cameras = world.query::<(&Camera, &Global3)>();
        let camera = if let Some((_, camera)) = cameras.iter().next() {
            camera
        } else {
            tracing::warn!("No camera found");
            return Ok(());
        };
        let camera_global = *camera.1;
        let camera_projection = camera.0.projection();
        drop(cameras);

        let mut encoder = None;

        // Create BLASes for new meshes.
        for (_, renderable) in
            world.query::<&Renderable>().with::<Global3>().iter()
        {
            match self.blases.entry(renderable.mesh.clone()) {
                Entry::Vacant(entry) => {
                    let blas = renderable.mesh.build_triangles_blas(
                        match &mut encoder {
                            Some(encoder) => encoder,
                            slot => {
                                *slot =
                                    Some(self.context.queue.create_encoder()?);
                                slot.as_mut().unwrap()
                            }
                        },
                        &self.context.device,
                        bump,
                    )?;

                    entry.insert(blas);
                }
                Entry::Occupied(_entry) => {}
            };
        }

        if let Some(encoder) = encoder {
            self.context
                .queue
                .submit_no_semaphores(encoder.finish(), None);
        }

        if self.frame > 1 {
            let fence = &self.fences[(self.frame % 2) as usize];
            self.device.wait_fences(&[fence], true);
            self.device.reset_fences(&[fence])
        }

        // self.pose.draw(
        //     (),
        //     self.frame,
        //     &[],
        //     &[],
        //     None,
        //     &mut self.context,
        //     world,
        //     bump,
        // )?;

        let frame = self
            .swapchain
            .acquire_image()?
            .expect("Resize unimplemented");

        let rt_prepass_output = self.rt_prepass.draw(
            rt_prepass::Input {
                extent: frame.info().image.info().extent.into_2d(),
                camera_global,
                camera_projection,
                blases: &self.blases,
            },
            self.frame,
            &[],
            &[],
            None,
            &mut self.context,
            world,
            bump,
        )?;

        if constants.filter_enabled {
            let diffuse_filter_output = self.diffuse_filter.draw(
                atrous::Input {
                    normal_depth: rt_prepass_output.normal_depth.clone(),
                    unfiltered: rt_prepass_output.diffuse,
                },
                self.frame,
                &[],
                &[],
                None,
                &mut self.context,
                world,
                bump,
            )?;

            let direct_filter_output = self.direct_filter.draw(
                atrous::Input {
                    normal_depth: rt_prepass_output.normal_depth.clone(),
                    unfiltered: rt_prepass_output.direct,
                },
                self.frame,
                &[],
                &[],
                None,
                &mut self.context,
                world,
                bump,
            )?;

            let fence = &self.fences[(self.frame % 2) as usize];
            self.combine.draw(
                combine::Input {
                    albedo: rt_prepass_output.albedo,
                    normal_depth: rt_prepass_output.normal_depth,
                    emissive: rt_prepass_output.emissive,
                    direct: direct_filter_output.filtered,
                    diffuse: diffuse_filter_output.filtered,
                    combined: frame.info().image.clone(),
                },
                self.frame,
                &[(
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    frame.info().wait.clone(),
                )],
                &[frame.info().signal.clone()],
                Some(fence),
                &mut self.context,
                world,
                bump,
            )?;
        } else {
            let fence = &self.fences[(self.frame % 2) as usize];
            self.combine.draw(
                combine::Input {
                    albedo: rt_prepass_output.albedo,
                    normal_depth: rt_prepass_output.normal_depth,
                    emissive: rt_prepass_output.emissive,
                    direct: rt_prepass_output.direct,
                    diffuse: rt_prepass_output.diffuse,
                    combined: frame.info().image.clone(),
                },
                self.frame,
                &[(
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    frame.info().wait.clone(),
                )],
                &[frame.info().signal.clone()],
                Some(fence),
                &mut self.context,
                world,
                bump,
            )?;
        }

        self.queue.present(frame);

        self.frame += 1;

        Ok(())
    }
}

struct BufferUpload {
    staging: Buffer,
    buffer: Buffer,
    offset: u64,
}

struct ImageUpload {
    staging: Buffer,
    image: Image,
    layout: Option<Layout>,
    row_length: u32,
    image_height: u32,
    subresource: ImageSubresourceLayers,
    offset: Offset3d,
    extent: Extent3d,
}

fn ray_tracing_transform_matrix_from_nalgebra(
    m: &na::Matrix4<f32>,
) -> TransformMatrix {
    let r = m.row(3);

    let ok = r[0].abs() < std::f32::EPSILON
        || r[1].abs() < std::f32::EPSILON
        || r[2].abs() < std::f32::EPSILON
        || (r[3] - 1.0).abs() < std::f32::EPSILON;

    if !ok {
        panic!("Matrix {} expected to have 0 0 0 1 bottom row");
    }

    let m = m.remove_row(3);

    TransformMatrix {
        matrix: m.transpose().into(),
    }
}

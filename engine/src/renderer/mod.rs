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
    crate::{camera::Camera, clocks::ClockIndex},
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::Pod,
    color_eyre::Report,
    eyre::{bail, eyre},
    hecs::World,
    std::{
        collections::hash_map::{Entry, HashMap},
        convert::TryFrom as _,
        ops::{Deref, DerefMut},
    },
    ultraviolet::{Isometry3, Mat4},
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
    pub mesh: Mesh,
    pub material: Material,
    pub transform: Option<Mat4>,
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
    ) -> Result<(), OutOfMemory>
    where
        T: Pod,
    {
        if buffer.info().memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.write_memory(buffer, offset, data);
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
        if info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.create_buffer_static(info, data)
        } else {
            info.usage |= BufferUsage::TRANSFER_DST;
            let buffer = self.device.create_buffer(info)?;
            self.upload_buffer(&buffer, 0, data)?;
            Ok(buffer)
        }

        // info.memory |= MemoryUsageFlags::UPLOAD;
        // self.device.create_buffer_static(info, data)
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

        // info.memory |= MemoryUsageFlags::UPLOAD;
        // self.device.create_image_static(info, data)
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

pub struct Renderer {
    context: Context,

    fences: [Fence; 2],

    frame: u64,

    blases: HashMap<Mesh, AccelerationStructure>,

    swapchain: Swapchain,
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

        // let blue_noise_upload = load_blue_noise_64x64x64(&device)?;
        tracing::trace!("Blue noise loaded");

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

        let rt_prepass = RtPrepass::new(
            window_extent,
            &mut context,
            // blue_noise_upload.buffer.clone(),
        )?;

        let mut swapchain = context.create_swapchain(&mut surface)?;
        swapchain.configure(
            ImageUsage::COLOR_ATTACHMENT,
            format,
            PresentMode::Fifo,
        )?;

        // let swapchain_blit = SwapchainBlitPresentPass;
        let combine = CombinePass::new(&mut context)?;
        let diffuse_filter = ATrousFilter::new(&mut context)?;
        let direct_filter = ATrousFilter::new(&mut context)?;

        // context.buffer_uploads.push(blue_noise_upload);

        Ok(Renderer {
            fences: [context.create_fence()?, context.create_fence()?],
            frame: 0,
            blases: HashMap::new(),
            swapchain,
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
        _clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<(), Report> {
        // let mut reg = Region::new();

        self.context.flush_uploads(bump)?;

        // tracing::info!("Uploads:\n{:#?}", reg.change_and_reset());

        tracing::debug!("Rendering next frame");

        let mut cameras = world.query::<(&Camera, &Isometry3)>();
        let camera = if let Some((_, camera)) = cameras.iter().next() {
            camera
        } else {
            tracing::warn!("No camera found");
            return Ok(());
        };
        let camera_isometry = *camera.1;
        let camera_projection = camera.0.projection();
        drop(cameras);

        // tracing::info!("Camera:\n{:#?}", reg.change_and_reset());

        let mut encoder = None;

        // Create BLASes for new meshes.
        for (_, renderable) in
            world.query::<&Renderable>().with::<Isometry3>().iter()
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

        // tracing::info!("BLASes:\n{:#?}", reg.change_and_reset());

        if self.frame > 1 {
            let fence = &self.fences[(self.frame % 2) as usize];
            self.device.wait_fences(&[fence], true);
            self.device.reset_fences(&[fence])
        }

        let frame = self
            .swapchain
            .acquire_image()?
            .expect("Resize unimplemented");

        // tracing::info!("Frame:\n{:#?}", reg.change_and_reset());

        let rt_prepass_output = self.rt_prepass.draw(
            rt_prepass::Input {
                extent: frame.info().image.info().extent.into_2d(),
                camera_transform: camera_isometry.into_homogeneous_matrix(),
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

        // tracing::info!("Prepass:\n{:#?}", reg.change_and_reset());

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
                // direct: rt_prepass_output.direct,
                // diffuse: rt_prepass_output.diffuse,
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

        // tracing::info!("Combine:\n{:#?}", reg.change_and_reset());

        self.queue.present(frame);

        // tracing::info!("Present:\n{:#?}", reg.change_and_reset());

        self.frame += 1;

        Ok(())
    }
}

// const BLUE_NOISE_PIXEL_COUNT: usize = 64 * 64 * 64 * 4;
// const BLUE_NOISE_SIZE: u64 = 64 * 64 * 64 * 16;

// fn load_blue_noise_64x64x64(
//     device: &Device,
// ) -> Result<BufferUpload, OutOfMemory> {
//     let images = [
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_0.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_1.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_2.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_3.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_4.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_5.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_6.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_7.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_8.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_9.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_10.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_11.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_12.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_13.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_14.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_15.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_16.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_17.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_18.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_19.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_20.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_21.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_22.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_23.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_24.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_25.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_26.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_27.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_28.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_29.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_30.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_31.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_32.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_33.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_34.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_35.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_36.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_37.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_38.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_39.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_40.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_41.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_42.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_43.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_44.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_45.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_46.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_47.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_48.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_49.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_50.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_51.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_52.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_53.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_54.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_55.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_56.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_57.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_58.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_59.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_60.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_61.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_62.png")[..],
//         &include_bytes!("../../blue_noise/64_64/HDR_RGBA_63.png")[..],
//     ];

//     let mut pixels = Vec::new();
//     let mut raw: Vec<u8> = Vec::new();

//     for &image in &images[..] {
//         let mut decoder = png::Decoder::new(image);
//         decoder.set_transformations(png::Transformations::IDENTITY);
//         let (info, mut reader) = decoder
//             .read_info()
//             .expect("Inlined png files expected to be valid");
//         assert_eq!(info.color_type, png::ColorType::RGBA);
//         assert_eq!(info.bit_depth, png::BitDepth::Sixteen);
//         const PIXEL_SIZE: usize = 8;
//         let size = usize::try_from(info.width).unwrap()
//             * usize::try_from(info.height).unwrap()
//             * PIXEL_SIZE;
//         assert_eq!(size, reader.output_buffer_size());
//         raw.resize(size, 0);
//         reader
//             .next_frame(&mut raw)
//             .expect("Inlined png files expected to be valid");

//         for pixel in raw.chunks(PIXEL_SIZE) {
//             match *cast_slice::<_, u16>(pixel) {
//                 [r, g, b, a] => {
//                     pixels.push(r as f32 / u16::max_value() as f32);
//                     pixels.push(g as f32 / u16::max_value() as f32);
//                     pixels.push(b as f32 / u16::max_value() as f32);
//                     pixels.push(a as f32 / u16::max_value() as f32);
//                 }
//                 _ => unreachable!(),
//             }
//         }
//     }

//     assert_eq!(pixels.len(), BLUE_NOISE_PIXEL_COUNT);

//     let staging = device.create_buffer_static(
//         BufferInfo {
//             align: 255,
//             size: BLUE_NOISE_SIZE,
//             usage: BufferUsage::TRANSFER_SRC,
//             memory: MemoryUsageFlags::UPLOAD,
//         },
//         &pixels,
//     )?;

//     let buffer = device.create_buffer(BufferInfo {
//         align: 255,
//         size: BLUE_NOISE_SIZE,
//         usage: BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
//         memory: MemoryUsageFlags::empty(),
//     })?;

//     Ok(BufferUpload {
//         staging,
//         buffer,
//         offset: 0,
//     })
// }

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

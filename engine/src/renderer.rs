mod material;
mod mesh;
// mod terrain;
mod pass;
mod vertex;

pub use self::{material::*, mesh::*, vertex::*};

use {
    self::pass::*,
    crate::{camera::Camera, clocks::ClockIndex, light::DirectionalLight},
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    color_eyre::Report,
    eyre::{bail, ensure, eyre, WrapErr as _},
    hecs::World,
    illume::*,
    std::{
        collections::hash_map::{Entry, HashMap},
        convert::TryFrom as _,
        mem::size_of,
        ops::{Deref, DerefMut},
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

pub struct Renderable;

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
            let buffer =
                self.device.create_buffer(info).map_err(|_| panic!())?;
            self.upload_buffer(&buffer, 0, data).map_err(|_| panic!())?;
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
        let image = self.device.create_image(info).map_err(|err| {
            panic!();
            err
        })?;
        self.upload_image(
            &image,
            None,
            row_length,
            image_height,
            subresource,
            Offset3d::ZERO,
            info.extent.into_3d(),
            data,
        )
        .map_err(|err| {
            panic!();
            err
        })?;
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
    swapchain_blit: SwapchainBlitPresentPass,
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

        let blue_noise_upload = load_blue_noise_64x64x64(&device)?;
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
            blue_noise_upload.buffer.clone(),
        )?;

        let mut swapchain = context.create_swapchain(&mut surface)?;
        swapchain.configure(
            ImageUsage::TRANSFER_DST,
            format,
            PresentMode::Mailbox,
        )?;

        let swapchain_blit = SwapchainBlitPresentPass;

        context.buffer_uploads.push(blue_noise_upload);

        Ok(Renderer {
            fences: [context.create_fence()?, context.create_fence()?],
            frame: 0,
            blases: HashMap::new(),
            rt_prepass,
            swapchain,
            swapchain_blit,
            context,
        })
    }

    pub fn draw(
        &mut self,
        world: &mut World,
        clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<(), Report> {
        self.context.flush_uploads(bump)?;

        tracing::debug!("Rendering next frame");

        let mut cameras = world.query::<(&Camera, &Mat4)>();
        let camera = if let Some((_, camera)) = cameras.iter().next() {
            camera
        } else {
            tracing::warn!("No camera found");
            return Ok(());
        };
        let camera_transform = *camera.1;
        let camera_projection = camera.0.projection();
        drop(cameras);

        let mut encoder = None;

        // Create BLASes for new meshes.
        let mut new_entities = BVec::with_capacity_in(32, bump);
        for (entity, mesh) in world
            .query::<&Mesh>()
            .with::<Mat4>()
            .without::<Renderable>()
            .iter()
        {
            match self.blases.entry(mesh.clone()) {
                Entry::Vacant(entry) => {
                    let blas = mesh.build_triangles_blas(
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

            new_entities.push((entity, Renderable));
        }

        if let Some(encoder) = encoder {
            self.context
                .queue
                .submit_no_semaphores(encoder.finish(), None);
        }

        // Insert them to the entities.
        for (entity, renderable) in new_entities {
            world.insert_one(entity, renderable).unwrap();
        }

        if self.frame > 1 {
            let fence = &self.fences[(self.frame % 2) as usize];
            self.device.wait_fences(&[fence], true);
            self.device.reset_fences(&[fence])
        }

        let frame = self
            .swapchain
            .acquire_image()?
            .expect("Resize unimplemented");

        let rt_prepass_output = self.rt_prepass.draw(
            rt_prepass::Input {
                extent: frame.info().image.info().extent.into_2d(),
                camera_transform,
                camera_projection,
                blases: &self.blases,
            },
            self.frame,
            None,
            &mut self.context,
            world,
            clock,
            bump,
        )?;

        let fence = &self.fences[(self.frame % 2) as usize];
        self.swapchain_blit.draw(
            swapchain::BlitInput {
                image: rt_prepass_output.output_albedo,
                frame,
            },
            self.frame,
            Some(fence),
            &mut self.context,
            world,
            clock,
            bump,
        )?;

        self.frame += 1;

        Ok(())
    }
}

const BLUE_NOISE_PIXEL_COUNT: usize = 64 * 64 * 64 * 4;
const BLUE_NOISE_SIZE: u64 = 64 * 64 * 64 * 16;

fn load_blue_noise_64x64x64(
    device: &Device,
) -> Result<BufferUpload, OutOfMemory> {
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

    let mut pixels = Vec::new();

    for &image in &images[..] {
        let image = load_from_memory_with_format(image, ImageFormat::Png)
            .unwrap()
            .to_rgba();

        for p in image.pixels() {
            let r = p[0] as f32 / 255.0;
            let g = p[1] as f32 / 255.0;
            let b = p[2] as f32 / 255.0;
            let a = p[3] as f32 / 255.0;

            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
    }

    assert_eq!(pixels.len(), BLUE_NOISE_PIXEL_COUNT);

    let staging = device.create_buffer_static(
        BufferInfo {
            align: 255,
            size: BLUE_NOISE_SIZE,
            usage: BufferUsage::TRANSFER_SRC,
            memory: MemoryUsageFlags::UPLOAD,
        },
        &pixels,
    )?;

    let buffer = device.create_buffer(BufferInfo {
        align: 255,
        size: BLUE_NOISE_SIZE,
        usage: BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
        memory: MemoryUsageFlags::empty(),
    })?;

    Ok(BufferUpload {
        staging,
        buffer,
        offset: 0,
    })
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

// Naive small bit set.
#[derive(Clone, Debug)]
struct BitSet {
    bits: u128,
}

impl BitSet {
    fn new() -> Self {
        BitSet { bits: !0 }
    }

    fn add(&mut self) -> Option<u32> {
        let index = self.bits.trailing_zeros();
        if index == 128 {
            None
        } else {
            self.bits &= !(1 << index);
            Some(index)
        }
    }

    fn unset(&mut self, index: u32) {
        let bit = 1 << index;
        debug_assert_eq!(self.bits & bit, 0);
        self.bits |= !bit;
    }
}

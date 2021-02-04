mod context;
mod material;
mod mesh;
mod pass;
mod pipeline;
mod vertex;

pub use {
    self::{context::Context, material::*, mesh::*, vertex::*},
    illume::*,
};

use {
    self::{pass::*, pipeline::*},
    crate::{
        camera::Camera, clocks::ClockIndex, resources::Resources,
        scene::Global3,
    },
    bumpalo::Bump,
    color_eyre::Report,
    eyre::eyre,
    hecs::World,
    nalgebra as na,
    std::{
        collections::hash_map::{Entry, HashMap},
        ops::{Deref, DerefMut},
    },
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
    blases: HashMap<Mesh, AccelerationStructure>,
    swapchain: Swapchain,
    swapchain_format: Format,
    blue_noise_buffer_256x256x128: Buffer,
    pipeline: PathTracePipeline,
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
                Feature::AccelerationStructure,
                Feature::RayTracingPipeline,
                Feature::BufferDeviceAddress,
                Feature::SurfacePresentation,
                Feature::RuntimeDescriptorArray,
                Feature::ScalarBlockLayout,
                Feature::DescriptorBindingUpdateUnusedWhilePending,
                Feature::DescriptorBindingPartiallyBound,
                Feature::ShaderSampledImageDynamicIndexing,
                Feature::ShaderSampledImageNonUniformIndexing,
                Feature::ShaderUniformBufferDynamicIndexing,
                Feature::ShaderUniformBufferNonUniformIndexing,
                Feature::ShaderStorageBufferDynamicIndexing,
                Feature::ShaderStorageBufferNonUniformIndexing,
            ],
            SingleQueueQuery::GENERAL,
        )?;

        tracing::debug!("{:?}", device);

        let swapchain_format = *surface_caps
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

        tracing::info!("Swapchain format: {:?}", swapchain_format);

        let mut context = Context::new(device, queue);

        let size = window.inner_size();
        let window_extent = Extent2d {
            width: size.width,
            height: size.height,
        };

        let mut swapchain = context.create_swapchain(&mut surface)?;
        swapchain.configure(
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
            swapchain_format,
            PresentMode::Fifo,
        )?;

        let blue_noise_buffer_256x256x128 = load_blue_noise(&mut context)?;

        let pipeline = PathTracePipeline::new(
            &mut context,
            blue_noise_buffer_256x256x128.clone(),
            Extent2d {
                width: 2560 / 4,
                height: 1080 / 4,
            },
        )?;

        Ok(Renderer {
            blases: HashMap::new(),
            swapchain,
            swapchain_format,
            context,
            blue_noise_buffer_256x256x128,
            pipeline,
        })
    }

    pub fn draw(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _clock: &ClockIndex,
        bump: &Bump,
    ) -> Result<(), Report> {
        const DEFAULT_CONSTANTS: RenderConstants = RenderConstants::new();

        let constants = &*resources.get_or(DEFAULT_CONSTANTS);

        self.context.flush_uploads(bump)?;

        tracing::debug!("Rendering next frame");

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

        tracing::trace!("BLASes created");

        if let Some(encoder) = encoder {
            self.context
                .queue
                .submit_no_semaphores(encoder.finish(), None);
        }

        let frame = loop {
            if let Some(frame) = self.swapchain.acquire_image()? {
                break frame;
            }
            tracing::error!("{:#?}", self.swapchain);
            self.swapchain.configure(
                ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                self.swapchain_format,
                PresentMode::Fifo,
            )?;
            tracing::error!("{:#?}", self.swapchain);
        };

        self.pipeline.draw(
            frame.info().image.clone(),
            &frame.info().wait,
            &frame.info().signal,
            &self.blases,
            &mut self.context,
            world,
            bump,
        )?;

        tracing::trace!("Presenting");
        match self.queue.present(frame) {
            Ok(PresentOk::Suboptimal) | Err(PresentError::OutOfDate) => {
                tracing::error!("{:#?}", self.swapchain);
                self.swapchain.configure(
                    ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                    self.swapchain_format,
                    PresentMode::Fifo,
                )?;
                tracing::error!("{:#?}", self.swapchain);
            }
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        };

        Ok(())
    }
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

fn load_blue_noise(ctx: &mut Context) -> Result<Buffer, OutOfMemory> {
    use wilds_noise::generate_blue_noise;

    let path = std::path::Path::new("blue_noise.tmp");
    if path.exists() {
        if path.is_file() {
            if let Ok(bytes) = std::fs::read(path) {
                return ctx
                    .create_buffer_static(
                        BufferInfo {
                            size: bytes.len() as _,
                            align: 255,
                            usage: BufferUsage::STORAGE,
                        },
                        &bytes[..],
                    )
                    .map(Into::into);
            }
        }
    }

    // Generate blue noise.
    const SIZE: usize = 256;
    const FMIN: f32 = 64.0;
    const FMAX: f32 = 256.0;
    const EXP: f32 = 1.5;

    let mut bytes = Vec::with_capacity(4 * 4 * 256 * 256 * 128);

    for _ in 0..128 {
        let red = generate_blue_noise(SIZE, FMIN, FMAX, EXP);
        let green = generate_blue_noise(SIZE, FMIN, FMAX, EXP);
        let blue = generate_blue_noise(SIZE, FMIN, FMAX, EXP);
        let alpha = generate_blue_noise(SIZE, FMIN, FMAX, EXP);

        let iter = red
            .values()
            .zip(green.values())
            .zip(blue.values())
            .zip(alpha.values());

        for (((r, g), b), a) in iter {
            bytes.extend_from_slice(&r.re.abs().fract().to_ne_bytes());
            bytes.extend_from_slice(&g.re.abs().fract().to_ne_bytes());
            bytes.extend_from_slice(&b.re.abs().fract().to_ne_bytes());
            bytes.extend_from_slice(&a.re.abs().fract().to_ne_bytes());
        }
    }

    if !path.exists() {
        let _ = std::fs::write(path, &bytes);
    }
    ctx.create_buffer_static(
        BufferInfo {
            size: bytes.len() as _,
            align: 255,
            usage: BufferUsage::STORAGE,
        },
        &bytes[..],
    )
    .map(Into::into)
}

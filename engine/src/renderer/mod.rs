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
    crate::{camera::Camera, clocks::ClockIndex, scene::Global3},
    bumpalo::Bump,
    color_eyre::Report,
    eyre::eyre,
    hecs::World,
    nalgebra as na,
    std::{
        collections::hash_map::{Entry, HashMap},
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
    blue_noise_buffer: Buffer,
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

        let mut context = Context::new(device, queue);

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

        // let pose = PosePass::new(&mut context)?;

        let blue_noise_buffer = load_blue_noise(&mut context)?;
        // let rt_prepass = RtPrepass::new(
        //     window_extent,
        //     &mut context,
        //     blue_noise_buffer.clone(),
        // )?;

        // let combine = CombinePass::new(&mut context)?;
        // let diffuse_filter = ATrousFilter::new(&mut context)?;
        // let direct_filter = ATrousFilter::new(&mut context)?;

        let pipeline = PathTracePipeline::new(
            &mut context,
            blue_noise_buffer.clone(),
            window_extent,
        )?;

        Ok(Renderer {
            blases: HashMap::new(),
            swapchain,
            context,
            blue_noise_buffer,
            pipeline,
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

        // if self.frame > 1 {
        //     let fence = &self.fences[(self.frame % 2) as usize];
        //     self.device.wait_fences(&[fence], true);
        //     self.device.reset_fences(&[fence])
        // }

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

        // let rt_prepass_output = self.rt_prepass.draw(
        //     rt_prepass::Input {
        //         extent: frame.info().image.info().extent.into_2d(),
        //         camera_global,
        //         camera_projection,
        //         blases: &self.blases,
        //     },
        //     self.frame,
        //     &[],
        //     &[],
        //     None,
        //     &mut self.context,
        //     world,
        //     bump,
        // )?;

        // if constants.filter_enabled {
        //     let diffuse_filter_output = self.diffuse_filter.draw(
        //         atrous::Input {
        //             normal_depth: rt_prepass_output.normal_depth.clone(),
        //             unfiltered: rt_prepass_output.diffuse,
        //         },
        //         self.frame,
        //         &[],
        //         &[],
        //         None,
        //         &mut self.context,
        //         world,
        //         bump,
        //     )?;

        //     let direct_filter_output = self.direct_filter.draw(
        //         atrous::Input {
        //             normal_depth: rt_prepass_output.normal_depth.clone(),
        //             unfiltered: rt_prepass_output.direct,
        //         },
        //         self.frame,
        //         &[],
        //         &[],
        //         None,
        //         &mut self.context,
        //         world,
        //         bump,
        //     )?;

        //     let fence = &self.fences[(self.frame % 2) as usize];
        //     self.combine.draw(
        //         combine::Input {
        //             albedo: rt_prepass_output.albedo,
        //             normal_depth: rt_prepass_output.normal_depth,
        //             emissive: rt_prepass_output.emissive,
        //             direct: direct_filter_output.filtered,
        //             diffuse: diffuse_filter_output.filtered,
        //             combined: frame.info().image.clone(),
        //         },
        //         self.frame,
        //         &[(
        //             PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //             frame.info().wait.clone(),
        //         )],
        //         &[frame.info().signal.clone()],
        //         Some(fence),
        //         &mut self.context,
        //         world,
        //         bump,
        //     )?;
        // } else {
        //     let fence = &self.fences[(self.frame % 2) as usize];
        //     self.combine.draw(
        //         combine::Input {
        //             albedo: rt_prepass_output.albedo,
        //             normal_depth: rt_prepass_output.normal_depth,
        //             emissive: rt_prepass_output.emissive,
        //             direct: rt_prepass_output.direct,
        //             diffuse: rt_prepass_output.diffuse,
        //             combined: frame.info().image.clone(),
        //         },
        //         self.frame,
        //         &[(
        //             PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //             frame.info().wait.clone(),
        //         )],
        //         &[frame.info().signal.clone()],
        //         Some(fence),
        //         &mut self.context,
        //         world,
        //         bump,
        //     )?;
        // }

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
        self.queue.present(frame);

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
    let blue_noise = include_bytes!("../../blue_noise/RGBAF32_256x256x128");

    ctx.create_buffer_static(
        BufferInfo {
            size: blue_noise.len() as _,
            align: 255,
            usage: BufferUsage::STORAGE,
            memory: MemoryUsageFlags::empty(),
        },
        &blue_noise[..],
    )
}

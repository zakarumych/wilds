//! Pipeline based on ideas in RTXGI.
use {
    super::Pipeline,
    crate::{
        camera::Camera,
        renderer::{
            pass::{
                ray_probe::{self, RayProbe},
                Pass as _,
            },
            AccelerationStructure, Buffer, Context, Extent2d, Fence, Image,
            Mesh, PipelineStageFlags, Semaphore,
        },
        scene::Global3,
    },
    bumpalo::Bump,
    eyre::Report,
    hecs::World,
    illume::*,
    std::collections::HashMap,
};

pub struct RayProbePipeline {
    ray_probe: RayProbe,

    frame: u64,
    fences: [Fence; 2],
}

impl RayProbePipeline {
    pub fn new(
        ctx: &mut Context,
        blue_noise_buffer_256x256x128: Buffer,
    ) -> Result<Self, Report> {
        let ray_probe = RayProbe::new(ctx, blue_noise_buffer_256x256x128)?;

        Ok(RayProbePipeline {
            ray_probe,

            frame: 0,
            fences: [ctx.create_fence()?, ctx.create_fence()?],
        })
    }
}

impl Pipeline for RayProbePipeline {
    fn draw(
        &mut self,
        target: Image,
        target_wait: &Semaphore,
        target_signal: &Semaphore,
        blases: &HashMap<Mesh, AccelerationStructure>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<(), Report> {
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

        if self.frame > 1 {
            let fence = &self.fences[(self.frame % 2) as usize];
            ctx.wait_fences(&[fence], true);
            ctx.reset_fences(&[fence])
        }

        let ray_probe_output = self.ray_probe.draw(
            ray_probe::Input {
                extent: target.info().extent.into_2d(),
                camera_global,
                camera_projection,
                blases,
            },
            self.frame,
            &[],
            &[],
            None,
            ctx,
            world,
            bump,
        )?;

        let rendered = ray_probe_output.output_image;
        let blit = ImageBlit {
            src_subresource: ImageSubresourceLayers::all_layers(
                rendered.info(),
                0,
            ),
            src_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(rendered.info().extent.into_3d())?,
            ],
            dst_subresource: ImageSubresourceLayers::all_layers(
                target.info(),
                0,
            ),
            dst_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(target.info().extent.into_3d())?,
            ],
        };

        let mut encoder = ctx.queue.create_encoder()?;

        let images = [
            ImageLayoutTransition::transition_whole(
                &rendered,
                Layout::General..Layout::TransferSrcOptimal,
            )
            .into(),
            ImageLayoutTransition::initialize_whole(
                &target,
                Layout::TransferDstOptimal,
            )
            .into(),
        ];

        encoder.image_barriers(
            PipelineStageFlags::RAY_TRACING_SHADER,
            PipelineStageFlags::TRANSFER
                | PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            &images,
        );

        encoder.blit_image(
            &rendered,
            Layout::TransferSrcOptimal,
            &target,
            Layout::TransferDstOptimal,
            std::slice::from_ref(&blit),
            Filter::Nearest,
        );

        let images = [ImageLayoutTransition::transition_whole(
            &target,
            Layout::TransferDstOptimal..Layout::Present,
        )
        .into()];

        encoder.image_barriers(
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::TOP_OF_PIPE,
            &images,
        );

        let fence = &self.fences[(self.frame % 2) as usize];
        ctx.queue.submit(
            &[(PipelineStageFlags::TRANSFER, target_wait.clone())],
            encoder.finish(),
            std::slice::from_ref(target_signal),
            Some(fence),
        );

        self.frame += 1;

        Ok(())
    }
}

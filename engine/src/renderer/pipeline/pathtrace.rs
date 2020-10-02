use {
    super::Pipeline,
    crate::{
        camera::Camera,
        renderer::{
            pass::{
                atrous::{self, ATrousFilter},
                combine::{self, CombinePass},
                rt_prepass::{self, RtPrepass},
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
    std::collections::HashMap,
};

pub struct PathTracePipeline {
    rt_prepass: RtPrepass,
    diffuse_filter: ATrousFilter,
    direct_filter: ATrousFilter,
    combine: CombinePass,

    frame: u64,
    fences: [Fence; 2],
}

impl PathTracePipeline {
    pub fn new(
        ctx: &mut Context,
        blue_noise_buffer: Buffer,
        target_extent: Extent2d,
    ) -> Result<Self, Report> {
        let rt_prepass = RtPrepass::new(target_extent, ctx, blue_noise_buffer)?;

        let combine = CombinePass::new(ctx)?;
        let diffuse_filter = ATrousFilter::new(ctx)?;
        let direct_filter = ATrousFilter::new(ctx)?;

        Ok(PathTracePipeline {
            rt_prepass,
            diffuse_filter,
            direct_filter,
            combine,

            frame: 0,
            fences: [ctx.create_fence()?, ctx.create_fence()?],
        })
    }
}

impl Pipeline for PathTracePipeline {
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

        let rt_prepass_output = self.rt_prepass.draw(
            rt_prepass::Input {
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

        let diffuse_filter_output = self.diffuse_filter.draw(
            atrous::Input {
                normal_depth: rt_prepass_output.normal_depth.clone(),
                unfiltered: rt_prepass_output.diffuse,
            },
            self.frame,
            &[],
            &[],
            None,
            ctx,
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
            ctx,
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
                combined: target.clone(),
            },
            self.frame,
            &[(
                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                target_wait.clone(),
            )],
            std::slice::from_ref(target_signal),
            Some(fence),
            ctx,
            world,
            bump,
        )?;

        self.frame += 1;

        Ok(())
    }
}

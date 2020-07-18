use {
    super::Pass,
    crate::{clocks::ClockIndex, renderer::Context},
    bumpalo::Bump,
    color_eyre::Report,
    hecs::World,
    illume::*,
};

pub struct SwapchainBlitPresentPass;

pub struct BlitInput {
    pub image: Image,
    pub frame: SwapchainImage,
}
pub struct Output;

impl Pass<'_> for SwapchainBlitPresentPass {
    type Input = BlitInput;
    type Output = Output;

    fn draw(
        &mut self,
        input: BlitInput,
        _frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        _world: &mut World,
        _clock: &ClockIndex,
        _bump: &Bump,
    ) -> Result<Output, Report> {
        let frame_image = &input.frame.info().image;
        let mut encoder = ctx.queue.create_encoder()?;

        // Sync swapchain image from transfer to presentation.
        let images = [
            ImageLayoutTransition::transition_whole(
                &input.image,
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
            PipelineStageFlags::all(),
            PipelineStageFlags::TRANSFER,
            &images,
        );

        // Blit ray-tracing result image to the frame.
        let blit = [ImageBlit {
            src_subresource: ImageSubresourceLayers::all_layers(
                input.image.info(),
                0,
            ),
            src_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(input.image.info().extent.into_3d())?,
            ],
            dst_subresource: ImageSubresourceLayers::all_layers(
                frame_image.info(),
                0,
            ),
            dst_offsets: [
                Offset3d::ZERO,
                Offset3d::from_extent(frame_image.info().extent.into_3d())?,
            ],
        }];

        encoder.blit_image(
            &input.image,
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

        // wait.iter().cloned().collect()

        // Submit execution.
        ctx.queue.submit(
            &[(PipelineStageFlags::all(), input.frame.info().wait.clone())],
            encoder.finish(),
            &[input.frame.info().signal.clone()],
            fence,
        );

        // Present the frame.
        ctx.queue.present(input.frame);

        Ok(Output)
    }
}

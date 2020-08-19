use {
    crate::{
        render_pass::{RenderPass, RENDERPASS_SMALLVEC_ATTACHMENTS},
        view::ImageView,
        Extent2d,
    },
    erupt::vk1_0,
    smallvec::SmallVec,
};

define_handle! {
    /// Framebuffer is a collection of attachments for render pass.
    /// Images format and sample count should match attachment definitions.
    /// All image views must be 2D with 1 mip level and 1 array level.
    pub struct Framebuffer {
        pub info: FramebufferInfo,
        handle: vk1_0::Framebuffer,
    }
}

#[derive(Clone, Debug, Hash)]
pub struct FramebufferInfo {
    pub render_pass: RenderPass,
    pub views: SmallVec<[ImageView; RENDERPASS_SMALLVEC_ATTACHMENTS]>,
    pub extent: Extent2d,
}

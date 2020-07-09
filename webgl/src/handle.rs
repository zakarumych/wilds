use crate::{device::WebGlDevice, image::WebGlImageInfo};
use illume::{
    resource::{Handle, ResourceTrait, Specific},
    Buffer, DescriptorSet, DescriptorSetLayout, Fence, Framebuffer,
    GraphicsPipeline, Image, ImageView, PipelineLayout, RenderPass, Semaphore,
    ShaderModule, Surface, SwapchainImage,
};
use std::{
    cell::{Cell, RefCell},
    sync::Weak,
};

pub(super) unsafe trait WebGlResource: ResourceTrait {
    type Owner: Sized;

    type WebGl: Specific<Self>;

    fn is_owner(&self, owner: &Self::Owner) -> bool;

    fn make(specific: Self::WebGl, info: Self::Info) -> Self {
        Self::from_handle(Handle::new(specific, info))
    }

    fn info(&self) -> &Self::Info {
        self.handle().info()
    }

    fn webgl_ref(&self, owner: &Self::Owner) -> &Self::WebGl {
        assert!(self.is_owner(owner), "Wrong owner");

        self.handle().specific_ref().expect("Wrong type")
    }

    unsafe fn webgl_ref_unchecked(&self) -> &Self::WebGl {
        self.handle().specific_ref_unchecked()
    }
}

#[derive(Debug)]

pub(super) struct WebGlSurface {
    pub(super) owner: WebGlDevice,
    pub(super) used: Cell<bool>,
}

impl Specific<Surface> for WebGlSurface {}

unsafe impl WebGlResource for Surface {
    type Owner = WebGlDevice;
    type WebGl = WebGlSurface;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| webgl.owner == *owner)
    }
}

#[derive(Debug)]

pub(super) struct WebGlBuffer {
    pub(super) handle: web_sys::WebGlBuffer,
    pub(super) owner: usize,
}

impl Specific<Buffer> for WebGlBuffer {}

unsafe impl WebGlResource for Buffer {
    type Owner = WebGlDevice;
    type WebGl = WebGlBuffer;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

pub(super) enum WebGlImage {
    Texture {
        handle: web_sys::WebGlTexture,
        info: WebGlImageInfo,
        owner: usize,
    },
    Renderbuffer {
        handle: web_sys::WebGlRenderbuffer,
        owner: usize,
    },
}

impl WebGlImage {
    pub fn texture(
        handle: web_sys::WebGlTexture,
        info: WebGlImageInfo,
        owner: usize,
    ) -> Self {
        Self::Texture {
            handle,
            info,
            owner,
        }
    }

    pub fn renderbuffer(
        handle: web_sys::WebGlRenderbuffer,
        owner: usize,
    ) -> Self {
        Self::Renderbuffer { handle, owner }
    }

    pub fn owner(&self) -> usize {
        match self {
            Self::Texture { owner, .. } => *owner,
            Self::Renderbuffer { owner, .. } => *owner,
        }
    }
}

impl std::fmt::Debug for WebGlImage {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Texture { handle, owner, .. } => fmt
                .debug_struct("WebGlImage")
                .field("handle", handle)
                .field("owner", &owner)
                .finish(),
            Self::Renderbuffer { handle, owner } => fmt
                .debug_struct("WebGlImage")
                .field("handle", handle)
                .field("owner", &owner)
                .finish(),
        }
    }
}

impl Specific<Image> for WebGlImage {}

unsafe impl WebGlResource for Image {
    type Owner = WebGlDevice;
    type WebGl = WebGlImage;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner()))
    }
}

#[derive(Debug)]

pub(super) struct WebGlImageView;

impl Specific<ImageView> for WebGlImageView {}

unsafe impl WebGlResource for ImageView {
    type Owner = WebGlDevice;
    type WebGl = WebGlImageView;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.info().image.is_owner(owner)
    }
}

// impl Specific<Buffer> for WebGlBuffer {}

// unsafe impl WebGlResource for Buffer {
//     type Owner = WebGlDevice;
//     type WebGl = WebGlBuffer;

//     fn is_owner(&self, owner: &Self::Owner) -> bool {
//         self.handle()
//             .specific_ref::<Self::WebGl>()
//             .and_then(|webgl| webgl.owner.upgrade())
//             .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
//     }
// }

#[derive(Debug)]

pub(super) struct WebGlSemaphore {
    pub(super) owner: usize,
}

impl Specific<Semaphore> for WebGlSemaphore {}

unsafe impl WebGlResource for Semaphore {
    type Owner = WebGlDevice;
    type WebGl = WebGlSemaphore;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Debug)]

pub(super) enum FenceState {
    Pending(web_sys::WebGlSync),
    Signalled,
    Unsignalled,
}

#[derive(Debug)]

pub(super) struct WebGlFence {
    pub(super) sync: RefCell<FenceState>,
    pub(super) owner: usize,
}

impl Specific<Fence> for WebGlFence {}

unsafe impl WebGlResource for Fence {
    type Owner = WebGlDevice;
    type WebGl = WebGlFence;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Debug)]

pub(super) struct WebGlColorClear {
    pub(super) index: usize,
    pub(super) clear: usize,
}

#[derive(Debug)]

pub(super) struct WebGlSubpassClears {
    pub(super) colors: Vec<WebGlColorClear>,
    pub(super) depth: Option<usize>,
}

#[derive(Debug)]

pub(super) struct WebGlRenderPass {
    pub(super) clears: Vec<WebGlSubpassClears>,
    pub(super) owner: usize,
}

impl Specific<RenderPass> for WebGlRenderPass {}

unsafe impl WebGlResource for RenderPass {
    type Owner = WebGlDevice;
    type WebGl = WebGlRenderPass;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Debug)]

pub(super) struct WebGlFramebuffer {
    pub(super) handles: Vec<web_sys::WebGlFramebuffer>,
    pub(super) owner: usize,
}

impl Specific<Framebuffer> for WebGlFramebuffer {}

unsafe impl WebGlResource for Framebuffer {
    type Owner = WebGlDevice;
    type WebGl = WebGlFramebuffer;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Debug)]

pub(super) struct WebGlShaderModule {
    pub(super) owner: usize,
}

impl Specific<ShaderModule> for WebGlShaderModule {}

unsafe impl WebGlResource for ShaderModule {
    type Owner = WebGlDevice;
    type WebGl = WebGlShaderModule;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

// #[derive(Debug)]
// pub(super) struct WebGlDescriptorSetLayout {
//     pub(super) handle: web_sys::WebGlDescriptorSetLayout,
//     pub(super) owner: usize,
//     pub(super) index: usize,
// }

// impl Specific<DescriptorSetLayout> for WebGlDescriptorSetLayout {}

// unsafe impl WebGlResource for DescriptorSetLayout {
//     type Owner = WebGlDevice;
//     type WebGl = WebGlDescriptorSetLayout;

//     fn is_owner(&self, owner: &Self::Owner) -> bool {
//         self.handle()
//             .specific_ref::<Self::WebGl>()
//             .and_then(|webgl| webgl.owner.upgrade())
//             .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
//     }
// }

// #[derive(Debug)]
// pub(super) struct WebGlDescriptorSet {
//     pub(super) handle: web_sys::WebGlDescriptorSet,
//     pub(super) owner: usize,
//     pub(super) index: usize,
// }

// impl Specific<DescriptorSet> for WebGlDescriptorSet {}

// unsafe impl WebGlResource for DescriptorSet {
//     type Owner = WebGlDevice;
//     type WebGl = WebGlDescriptorSet;

//     fn is_owner(&self, owner: &Self::Owner) -> bool {
//         self.handle()
//             .specific_ref::<Self::WebGl>()
//             .and_then(|webgl| webgl.owner.upgrade())
//             .map_or(false, |real_owner| std::ptr::eq(&*real_owner, owner))
//     }
// }

#[derive(Debug)]

pub(super) struct WebGlPipelineLayout {
    pub(super) owner: usize,
}

impl Specific<PipelineLayout> for WebGlPipelineLayout {}

unsafe impl WebGlResource for PipelineLayout {
    type Owner = WebGlDevice;
    type WebGl = WebGlPipelineLayout;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Debug)]

pub(super) struct WebGlGraphicsPipeline {
    pub(super) program: web_sys::WebGlProgram,
    pub(super) owner: usize,
}

impl Specific<GraphicsPipeline> for WebGlGraphicsPipeline {}

unsafe impl WebGlResource for GraphicsPipeline {
    type Owner = WebGlDevice;
    type WebGl = WebGlGraphicsPipeline;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

#[derive(Clone, Debug)]

pub(super) struct WebGlSwapchainImage {
    pub(super) renderbuffer: web_sys::WebGlRenderbuffer,
    pub(super) framebuffer: web_sys::WebGlFramebuffer,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) owner: usize,
}

impl Specific<SwapchainImage> for WebGlSwapchainImage {}

unsafe impl WebGlResource for SwapchainImage {
    type Owner = WebGlDevice;
    type WebGl = WebGlSwapchainImage;

    fn is_owner(&self, owner: &Self::Owner) -> bool {
        self.handle()
            .specific_ref::<Self::WebGl>()
            .map_or(false, |webgl| owner.is(webgl.owner))
    }
}

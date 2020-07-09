use crate::device::WebGlDevice;
use illume::{format::Format, image::ImageUsage};

#[derive(Clone, Copy, Debug)]
#[allow(non_camel_case_types)]

pub(super) enum Repr {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F16,
    F32,
    U24_8,
    F32_U24_8,
}

impl Repr {
    pub fn into_webgl(self) -> u32 {
        type GL = web_sys::WebGl2RenderingContext;

        match self {
            Self::U8 => GL::UNSIGNED_BYTE,
            Self::I8 => GL::BYTE,
            Self::U16 => GL::UNSIGNED_SHORT,
            Self::I16 => GL::SHORT,
            Self::U32 => GL::UNSIGNED_INT,
            Self::I32 => GL::INT,
            Self::F16 => GL::HALF_FLOAT,
            Self::F32 => GL::FLOAT,
            Self::U24_8 => GL::UNSIGNED_INT_24_8,
            Self::F32_U24_8 => GL::FLOAT_32_UNSIGNED_INT_24_8_REV,
        }
    }
}

pub(super) struct WebGlColorFormat {
    pub internal: u32,
    pub format: u32,
    pub repr: Repr,
    pub texture_usage: ImageUsage,
    pub render_usage: ImageUsage,
    pub filterable: bool,
}

const COPY: ImageUsage = ImageUsage::from_bits_truncate(
    ImageUsage::TRANSFER_SRC.bits() | ImageUsage::TRANSFER_DST.bits(),
);

const SHADER_INPUT: ImageUsage = ImageUsage::from_bits_truncate(
    ImageUsage::TRANSFER_SRC.bits()
        | ImageUsage::TRANSFER_DST.bits()
        | ImageUsage::SAMPLED.bits()
        | ImageUsage::STORAGE.bits()
        | ImageUsage::INPUT_ATTACHMENT.bits(),
);

const COLOR_RENDERABLE: ImageUsage = ImageUsage::from_bits_truncate(
    ImageUsage::TRANSFER_SRC.bits()
        | ImageUsage::TRANSFER_DST.bits()
        | ImageUsage::COLOR_ATTACHMENT.bits()
        | ImageUsage::TRANSIENT_ATTACHMENT.bits(),
);

const COLOR_RENDERABLE_SHADER_INPUT: ImageUsage =
    ImageUsage::from_bits_truncate(
        ImageUsage::TRANSFER_SRC.bits()
            | ImageUsage::TRANSFER_DST.bits()
            | ImageUsage::SAMPLED.bits()
            | ImageUsage::STORAGE.bits()
            | ImageUsage::INPUT_ATTACHMENT.bits()
            | ImageUsage::COLOR_ATTACHMENT.bits()
            | ImageUsage::TRANSIENT_ATTACHMENT.bits(),
    );

const DEPTH_STENCIL_RENDERABLE: ImageUsage = ImageUsage::from_bits_truncate(
    ImageUsage::TRANSFER_SRC.bits()
        | ImageUsage::TRANSFER_DST.bits()
        | ImageUsage::DEPTH_STENCIL_ATTACHMENT.bits()
        | ImageUsage::TRANSIENT_ATTACHMENT.bits(),
);

const DEPTH_STENCIL_RENDERABLE_SHADER_INPUT: ImageUsage =
    ImageUsage::from_bits_truncate(
        ImageUsage::TRANSFER_SRC.bits()
            | ImageUsage::TRANSFER_DST.bits()
            | ImageUsage::SAMPLED.bits()
            | ImageUsage::STORAGE.bits()
            | ImageUsage::INPUT_ATTACHMENT.bits()
            | ImageUsage::DEPTH_STENCIL_ATTACHMENT.bits()
            | ImageUsage::TRANSIENT_ATTACHMENT.bits(),
    );

fn ext_renderable(ctx: &WebGlDevice) -> ImageUsage {
    if ctx.has_extension("EXT_color_buffer_float") {
        ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT
    } else {
        ImageUsage::empty()
    }
}

fn ext_texture(ctx: &WebGlDevice) -> ImageUsage {
    if ctx.has_extension("WEBGL_depth_texture") {
        SHADER_INPUT
    } else {
        ImageUsage::empty()
    }
}

pub(super) fn texture_format_to_webgl(
    ctx: &WebGlEruptDevice,
    format: Format,
) -> Option<WebGlColorFormat> {
    use Format::*;

    type GL = web_sys::WebGl2RenderingContext;

    assert!(format.is_color(), "Color format expected");

    Some(match format {
        R8Unorm => WebGlColorFormat {
            internal: GL::R8,
            format: GL::RED,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: true,
        },
        R8Snorm => WebGlColorFormat {
            internal: GL::R8_SNORM,
            format: GL::RED,
            repr: Repr::I8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        R8Uint => WebGlColorFormat {
            internal: GL::R8UI,
            format: GL::RED_INTEGER,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        R8Sint => WebGlColorFormat {
            internal: GL::R8I,
            format: GL::RED_INTEGER,
            repr: Repr::I8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG8Unorm => WebGlColorFormat {
            internal: GL::RG8,
            format: GL::RG,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: true,
        },
        RG8Snorm => WebGlColorFormat {
            internal: GL::RG8_SNORM,
            format: GL::RG,
            repr: Repr::I8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        RG8Uint => WebGlColorFormat {
            internal: GL::RG8UI,
            format: GL::RG_INTEGER,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG8Sint => WebGlColorFormat {
            internal: GL::RG8I,
            format: GL::RG_INTEGER,
            repr: Repr::I8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGB8Unorm => WebGlColorFormat {
            internal: GL::RGB8,
            format: GL::RGB,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: true,
        },
        RGB8Snorm => WebGlColorFormat {
            internal: GL::RGB8_SNORM,
            format: GL::RGB,
            repr: Repr::I8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        RGB8Uint => WebGlColorFormat {
            internal: GL::RGB8UI,
            format: GL::RGB_INTEGER,
            repr: Repr::U8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGB8Sint => WebGlColorFormat {
            internal: GL::RGB8I,
            format: GL::RGB_INTEGER,
            repr: Repr::I8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGB8Srgb => WebGlColorFormat {
            internal: GL::SRGB8,
            format: GL::RGB,
            repr: Repr::U8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        RGBA8Unorm => WebGlColorFormat {
            internal: GL::RGBA8,
            format: GL::RGBA,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: true,
        },
        RGBA8Snorm => WebGlColorFormat {
            internal: GL::RGBA8_SNORM,
            format: GL::RGBA,
            repr: Repr::I8,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        RGBA8Uint => WebGlColorFormat {
            internal: GL::RGBA8UI,
            format: GL::RGBA_INTEGER,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGBA8Sint => WebGlColorFormat {
            internal: GL::RGBA8I,
            format: GL::RGBA_INTEGER,
            repr: Repr::I8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGBA8Srgb => WebGlColorFormat {
            internal: GL::SRGB8_ALPHA8,
            format: GL::RGBA,
            repr: Repr::U8,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: true,
        },
        R16Uint => WebGlColorFormat {
            internal: GL::R16UI,
            format: GL::RED,
            repr: Repr::U16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        R16Sint => WebGlColorFormat {
            internal: GL::R16I,
            format: GL::RED,
            repr: Repr::I16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG16Uint => WebGlColorFormat {
            internal: GL::RG16UI,
            format: GL::RG,
            repr: Repr::U16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG16Sint => WebGlColorFormat {
            internal: GL::RG16I,
            format: GL::RG,
            repr: Repr::I16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGB16Uint => WebGlColorFormat {
            internal: GL::RGB16UI,
            format: GL::RGB,
            repr: Repr::U16,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGB16Sint => WebGlColorFormat {
            internal: GL::RGB16I,
            format: GL::RGB,
            repr: Repr::I16,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGBA16Uint => WebGlColorFormat {
            internal: GL::RGBA16UI,
            format: GL::RGBA,
            repr: Repr::U16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGBA16Sint => WebGlColorFormat {
            internal: GL::RGBA16I,
            format: GL::RGBA,
            repr: Repr::I16,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        R32Uint => WebGlColorFormat {
            internal: GL::R32UI,
            format: GL::RED,
            repr: Repr::U32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        R32Sint => WebGlColorFormat {
            internal: GL::R32I,
            format: GL::RED,
            repr: Repr::I32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG32Uint => WebGlColorFormat {
            internal: GL::RG32UI,
            format: GL::RG,
            repr: Repr::U32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RG32Sint => WebGlColorFormat {
            internal: GL::RG32I,
            format: GL::RG,
            repr: Repr::I32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGB32Uint => WebGlColorFormat {
            internal: GL::RGB32UI,
            format: GL::RGB,
            repr: Repr::U32,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGB32Sint => WebGlColorFormat {
            internal: GL::RGB32I,
            format: GL::RGB,
            repr: Repr::I32,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGBA32Uint => WebGlColorFormat {
            internal: GL::RGBA32UI,
            format: GL::RGBA,
            repr: Repr::U32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        RGBA32Sint => WebGlColorFormat {
            internal: GL::RGBA32I,
            format: GL::RGBA,
            repr: Repr::I32,
            texture_usage: COLOR_RENDERABLE_SHADER_INPUT,
            render_usage: COLOR_RENDERABLE,
            filterable: false,
        },
        R16Sfloat => WebGlColorFormat {
            internal: GL::R16F,
            format: GL::RED,
            repr: Repr::F16,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: true,
        },
        RG16Sfloat => WebGlColorFormat {
            internal: GL::RG16F,
            format: GL::RG,
            repr: Repr::F16,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: true,
        },
        RGB16Sfloat => WebGlColorFormat {
            internal: GL::RGB16F,
            format: GL::RGB,
            repr: Repr::F16,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: true,
        },
        RGBA16Sfloat => WebGlColorFormat {
            internal: GL::RGBA16F,
            format: GL::RGBA,
            repr: Repr::F16,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: true,
        },
        R32Sfloat => WebGlColorFormat {
            internal: GL::R32F,
            format: GL::RED,
            repr: Repr::F32,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: false,
        },
        RG32Sfloat => WebGlColorFormat {
            internal: GL::RG32F,
            format: GL::RG,
            repr: Repr::F32,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: false,
        },
        RGB32Sfloat => WebGlColorFormat {
            internal: GL::RGB32F,
            format: GL::RGB,
            repr: Repr::F32,
            texture_usage: SHADER_INPUT,
            render_usage: COPY,
            filterable: false,
        },
        RGBA32Sfloat => WebGlColorFormat {
            internal: GL::RGBA32F,
            format: GL::RGBA,
            repr: Repr::F32,
            texture_usage: SHADER_INPUT | ext_renderable(ctx),
            render_usage: COPY | ext_renderable(ctx),
            filterable: false,
        },
        D16Unorm => WebGlColorFormat {
            internal: GL::DEPTH_COMPONENT16,
            format: GL::DEPTH_COMPONENT,
            repr: Repr::U16,
            texture_usage: DEPTH_STENCIL_RENDERABLE | ext_texture(ctx),
            render_usage: DEPTH_STENCIL_RENDERABLE,
            filterable: false,
        },
        D32Sfloat => WebGlColorFormat {
            internal: GL::DEPTH_COMPONENT32F,
            format: GL::DEPTH_COMPONENT,
            repr: Repr::F32,
            texture_usage: DEPTH_STENCIL_RENDERABLE | ext_texture(ctx),
            render_usage: DEPTH_STENCIL_RENDERABLE,
            filterable: false,
        },
        D24UnormS8Uint => WebGlColorFormat {
            internal: GL::DEPTH24_STENCIL8,
            format: GL::DEPTH_STENCIL,
            repr: Repr::U24_8,
            texture_usage: DEPTH_STENCIL_RENDERABLE | ext_texture(ctx),
            render_usage: DEPTH_STENCIL_RENDERABLE,
            filterable: false,
        },
        D32SfloatS8Uint => WebGlColorFormat {
            internal: GL::DEPTH32F_STENCIL8,
            format: GL::DEPTH_STENCIL,
            repr: Repr::F32_U24_8,
            texture_usage: DEPTH_STENCIL_RENDERABLE | ext_texture(ctx),
            render_usage: DEPTH_STENCIL_RENDERABLE,
            filterable: false,
        },
        _ => return None,
    })
}

use crate::device::WebGlDevice;
use illume::{
    device::CreateImageError,
    format::Format,
    image::{ImageExtent, ImageInfo, ImageUsage, Samples},
};

#[derive(Debug)]

pub(super) enum WebGlImageKind {
    Texture,
    Renderbuffer,
}

#[derive(Debug)]

pub(super) struct WebGlImageInfo {
    pub internal: u32,
    pub format: u32,
    pub repr: u32,
    pub filterable: bool,
}

pub(super) fn webgl_image_info(
    ctx: &WebGlDevice,
    info: &ImageInfo,
    texture_only: bool,
) -> Option<(WebGlImageInfo, WebGlImageKind)> {
    use Format::*;

    type GL = web_sys::WebGl2RenderingContext;

    let copy_usage: ImageUsage =
        ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST;

    let shader_input_usage: ImageUsage = ImageUsage::SAMPLED
        | ImageUsage::STORAGE
        | ImageUsage::INPUT_ATTACHMENT;

    let ext_color_renderable = if ctx.has_extension("EXT_color_buffer_float") {
        ImageUsage::COLOR_ATTACHMENT
    } else {
        ImageUsage::empty()
    };

    let depth_texture = if ctx.has_extension("WEBGL_depth_texture") {
        ImageUsage::SAMPLED | ImageUsage::STORAGE | ImageUsage::INPUT_ATTACHMENT
    } else {
        ImageUsage::empty()
    };

    let internal: u32;

    let format: u32;

    let repr: u32;

    let texture_usage: ImageUsage;

    let render_usage: ImageUsage;

    let filterable: bool;

    match info.format {
        R8Unorm => {
            internal = GL::R8;

            format = GL::RED;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = true;
        }
        R8Snorm => {
            internal = GL::R8_SNORM;

            format = GL::RED;

            repr = GL::BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        R8Uint => {
            internal = GL::R8UI;

            format = GL::RED_INTEGER;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        R8Sint => {
            internal = GL::R8I;

            format = GL::RED_INTEGER;

            repr = GL::BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG8Unorm => {
            internal = GL::RG8;

            format = GL::RG;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = true;
        }
        RG8Snorm => {
            internal = GL::RG8_SNORM;

            format = GL::RG;

            repr = GL::BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        RG8Uint => {
            internal = GL::RG8UI;

            format = GL::RG_INTEGER;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG8Sint => {
            internal = GL::RG8I;

            format = GL::RG_INTEGER;

            repr = GL::BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGB8Unorm => {
            internal = GL::RGB8;

            format = GL::RGB;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = true;
        }
        RGB8Snorm => {
            internal = GL::RGB8_SNORM;

            format = GL::RGB;

            repr = GL::BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        RGB8Uint => {
            internal = GL::RGB8UI;

            format = GL::RGB_INTEGER;

            repr = GL::UNSIGNED_BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGB8Sint => {
            internal = GL::RGB8I;

            format = GL::RGB_INTEGER;

            repr = GL::BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGB8Srgb => {
            internal = GL::SRGB8;

            format = GL::RGB;

            repr = GL::UNSIGNED_BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        RGBA8Unorm => {
            internal = GL::RGBA8;

            format = GL::RGBA;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = true;
        }
        RGBA8Snorm => {
            internal = GL::RGBA8_SNORM;

            format = GL::RGBA;

            repr = GL::BYTE;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        RGBA8Uint => {
            internal = GL::RGBA8UI;

            format = GL::RGBA_INTEGER;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGBA8Sint => {
            internal = GL::RGBA8I;

            format = GL::RGBA_INTEGER;

            repr = GL::BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGBA8Srgb => {
            internal = GL::SRGB8_ALPHA8;

            format = GL::RGBA;

            repr = GL::UNSIGNED_BYTE;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = true;
        }
        R16Uint => {
            internal = GL::R16UI;

            format = GL::RED;

            repr = GL::UNSIGNED_SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        R16Sint => {
            internal = GL::R16I;

            format = GL::RED;

            repr = GL::SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG16Uint => {
            internal = GL::RG16UI;

            format = GL::RG;

            repr = GL::UNSIGNED_SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG16Sint => {
            internal = GL::RG16I;

            format = GL::RG;

            repr = GL::SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGB16Uint => {
            internal = GL::RGB16UI;

            format = GL::RGB;

            repr = GL::UNSIGNED_SHORT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGB16Sint => {
            internal = GL::RGB16I;

            format = GL::RGB;

            repr = GL::SHORT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGBA16Uint => {
            internal = GL::RGBA16UI;

            format = GL::RGBA;

            repr = GL::UNSIGNED_SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGBA16Sint => {
            internal = GL::RGBA16I;

            format = GL::RGBA;

            repr = GL::SHORT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        R32Uint => {
            internal = GL::R32UI;

            format = GL::RED;

            repr = GL::UNSIGNED_INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        R32Sint => {
            internal = GL::R32I;

            format = GL::RED;

            repr = GL::INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG32Uint => {
            internal = GL::RG32UI;

            format = GL::RG;

            repr = GL::UNSIGNED_INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RG32Sint => {
            internal = GL::RG32I;

            format = GL::RG;

            repr = GL::INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGB32Uint => {
            internal = GL::RGB32UI;

            format = GL::RGB;

            repr = GL::UNSIGNED_INT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGB32Sint => {
            internal = GL::RGB32I;

            format = GL::RGB;

            repr = GL::INT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGBA32Uint => {
            internal = GL::RGBA32UI;

            format = GL::RGBA;

            repr = GL::UNSIGNED_INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        RGBA32Sint => {
            internal = GL::RGBA32I;

            format = GL::RGBA;

            repr = GL::INT;

            texture_usage =
                ImageUsage::COLOR_ATTACHMENT | shader_input_usage | copy_usage;

            render_usage = ImageUsage::COLOR_ATTACHMENT;

            filterable = false;
        }
        R16Sfloat => {
            internal = GL::R16F;

            format = GL::RED;

            repr = GL::HALF_FLOAT;

            texture_usage =
                ext_color_renderable | shader_input_usage | copy_usage;

            render_usage = copy_usage | ext_color_renderable;

            filterable = true;
        }
        RG16Sfloat => {
            internal = GL::RG16F;

            format = GL::RG;

            repr = GL::HALF_FLOAT;

            texture_usage = shader_input_usage | ext_color_renderable;

            render_usage = copy_usage | ext_color_renderable;

            filterable = true;
        }
        RGB16Sfloat => {
            internal = GL::RGB16F;

            format = GL::RGB;

            repr = GL::HALF_FLOAT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = true;
        }
        RGBA16Sfloat => {
            internal = GL::RGBA16F;

            format = GL::RGBA;

            repr = GL::HALF_FLOAT;

            texture_usage = shader_input_usage | ext_color_renderable;

            render_usage = copy_usage | ext_color_renderable;

            filterable = true;
        }
        R32Sfloat => {
            internal = GL::R32F;

            format = GL::RED;

            repr = GL::FLOAT;

            texture_usage = shader_input_usage | ext_color_renderable;

            render_usage = copy_usage | ext_color_renderable;

            filterable = false;
        }
        RG32Sfloat => {
            internal = GL::RG32F;

            format = GL::RG;

            repr = GL::FLOAT;

            texture_usage = shader_input_usage | ext_color_renderable;

            render_usage = copy_usage | ext_color_renderable;

            filterable = false;
        }
        RGB32Sfloat => {
            internal = GL::RGB32F;

            format = GL::RGB;

            repr = GL::FLOAT;

            texture_usage = shader_input_usage;

            render_usage = copy_usage;

            filterable = false;
        }
        RGBA32Sfloat => {
            internal = GL::RGBA32F;

            format = GL::RGBA;

            repr = GL::FLOAT;

            texture_usage = shader_input_usage | ext_color_renderable;

            render_usage = copy_usage | ext_color_renderable;

            filterable = false;
        }
        D16Unorm => {
            internal = GL::DEPTH_COMPONENT16;

            format = GL::DEPTH_COMPONENT;

            repr = GL::UNSIGNED_SHORT;

            texture_usage =
                ImageUsage::DEPTH_STENCIL_ATTACHMENT | depth_texture;

            render_usage = ImageUsage::DEPTH_STENCIL_ATTACHMENT;

            filterable = false;
        }
        D32Sfloat => {
            internal = GL::DEPTH_COMPONENT32F;

            format = GL::DEPTH_COMPONENT;

            repr = GL::FLOAT;

            texture_usage =
                ImageUsage::DEPTH_STENCIL_ATTACHMENT | depth_texture;

            render_usage = ImageUsage::DEPTH_STENCIL_ATTACHMENT;

            filterable = false;
        }
        D24UnormS8Uint => {
            internal = GL::DEPTH24_STENCIL8;

            format = GL::DEPTH_STENCIL;

            repr = GL::UNSIGNED_INT_24_8;

            texture_usage =
                ImageUsage::DEPTH_STENCIL_ATTACHMENT | depth_texture;

            render_usage = ImageUsage::DEPTH_STENCIL_ATTACHMENT;

            filterable = false;
        }
        D32SfloatS8Uint => {
            internal = GL::DEPTH32F_STENCIL8;

            format = GL::DEPTH_STENCIL;

            repr = GL::FLOAT_32_UNSIGNED_INT_24_8_REV;

            texture_usage =
                ImageUsage::DEPTH_STENCIL_ATTACHMENT | depth_texture;

            render_usage = ImageUsage::DEPTH_STENCIL_ATTACHMENT;

            filterable = false;
        }
        _ => return None,
    }

    let is_3d = match info.extent {
        ImageExtent::D3 { .. } => true,
        _ => false,
    };

    let webgl_info = WebGlImageInfo {
        internal,
        format,
        repr,
        filterable,
    };

    if is_3d {
        if texture_usage.contains(info.usage)
            && !info.usage.intersects(
                ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            )
        {
            if let Samples::Samples1 = info.samples {
                Some((webgl_info, WebGlImageKind::Texture))
            } else {
                None
            }
        } else {
            None
        }
    } else if texture_only {
        if texture_usage.contains(info.usage) {
            Some((webgl_info, WebGlImageKind::Texture))
        } else {
            None
        }
    } else if let Samples::Samples1 = info.samples {
        if copy_usage.contains(info.usage) {
            Some((webgl_info, WebGlImageKind::Texture))
        } else if render_usage.contains(info.usage) {
            Some((webgl_info, WebGlImageKind::Renderbuffer))
        } else if texture_usage.contains(info.usage) {
            Some((webgl_info, WebGlImageKind::Texture))
        } else {
            None
        }
    } else {
        if render_usage.contains(info.usage) {
            Some((webgl_info, WebGlImageKind::Renderbuffer))
        } else {
            None
        }
    }
}

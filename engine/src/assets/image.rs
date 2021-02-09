use {
    crate::renderer::Context,
    goods::{AssetDefaultFormat, SimpleFormat, SyncAsset},
    illume::{
        CreateImageError, ImageExtent, ImageInfo, ImageUsage, ImageView,
        ImageViewInfo, Samples1,
    },
    image::{load_from_memory, DynamicImage, GenericImageView as _},
};

/// Image asset.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ImageAsset {
    pub image: ImageView,
}

impl ImageAsset {
    pub fn into_inner(self) -> ImageView {
        self.image
    }
}

impl SyncAsset for ImageAsset {
    type Context = Context;
    type Repr = DynamicImage;

    fn build(image: DynamicImage, ctx: &mut Context) -> eyre::Result<Self> {
        let image = image.into_rgba8();
        let image = image_view_from_dyn_image(
            &image::DynamicImage::ImageRgba8(image),
            ctx,
        )?;

        Ok(ImageAsset { image })
    }
}

/// Quasi-format that tries to guess image format.
#[derive(Debug, Default)]
pub struct GuessImageFormat;

impl<K> SimpleFormat<DynamicImage, K> for GuessImageFormat {
    fn decode_simple(
        self,
        _key: K,
        bytes: Box<[u8]>,
    ) -> eyre::Result<DynamicImage> {
        load_from_memory(&bytes).map_err(eyre::Report::from)
    }
}

impl AssetDefaultFormat for ImageAsset {
    type DefaultFormat = GuessImageFormat;
}

pub fn image_view_from_dyn_image(
    image: &DynamicImage,
    ctx: &mut Context,
) -> Result<ImageView, CreateImageError> {
    use illume::Format;

    let format = match &image {
        DynamicImage::ImageLuma8(_) => Format::R8Unorm,
        DynamicImage::ImageLumaA8(_) => Format::RG8Unorm,
        DynamicImage::ImageRgb8(_) => Format::RGB8Unorm,
        DynamicImage::ImageRgba8(_) => Format::RGBA8Unorm,
        DynamicImage::ImageBgr8(_) => Format::BGR8Unorm,
        DynamicImage::ImageBgra8(_) => Format::BGRA8Unorm,
        DynamicImage::ImageLuma16(_) => Format::R16Unorm,
        DynamicImage::ImageLumaA16(_) => Format::RG16Unorm,
        DynamicImage::ImageRgb16(_) => Format::RGB16Unorm,
        DynamicImage::ImageRgba16(_) => Format::RGBA16Unorm,
    };

    let (w, h) = image.dimensions();

    let bytes8;
    let bytes16;

    let bytes = match image {
        DynamicImage::ImageLuma8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageLumaA8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageRgb8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageRgba8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageBgr8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageBgra8(buffer) => {
            bytes8 = &**buffer;
            &bytes8[..]
        }
        DynamicImage::ImageLuma16(buffer) => {
            bytes16 = &**buffer;
            bytemuck::cast_slice(&bytes16[..])
        }
        DynamicImage::ImageLumaA16(buffer) => {
            bytes16 = &**buffer;
            bytemuck::cast_slice(&bytes16[..])
        }
        DynamicImage::ImageRgb16(buffer) => {
            bytes16 = &**buffer;
            bytemuck::cast_slice(&bytes16[..])
        }
        DynamicImage::ImageRgba16(buffer) => {
            bytes16 = &**buffer;
            bytemuck::cast_slice(&bytes16[..])
        }
    };
    let image = ctx.create_image_static(
        ImageInfo {
            extent: ImageExtent::D2 {
                width: w,
                height: h,
            },
            format,
            levels: 1,
            layers: 1,
            samples: Samples1,
            usage: ImageUsage::SAMPLED,
        },
        0,
        0,
        &bytes,
    )?;

    let view = ctx.create_image_view(ImageViewInfo::new(image))?;
    Ok(view)
}

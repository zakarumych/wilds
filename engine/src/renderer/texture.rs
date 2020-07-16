use {
    super::Context,
    goods::{ready, Cache, Format, Ready, SyncAsset},
    illume::{
        CreateImageError, Device, Image, ImageExtent, ImageInfo, ImageUsage,
        Samples1,
    },
    image::{
        load_from_memory, DynamicImage, GenericImageView as _, ImageError,
    },
};

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Texture(pub Image);

impl SyncAsset for Texture {
    type Context = Context;
    type Error = CreateImageError;
    type Repr = DynamicImage;

    fn build(
        image: DynamicImage,
        ctx: &mut Context,
    ) -> Result<Self, CreateImageError> {
        use illume::Format;

        let format = match image {
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
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageLumaA8(buffer) => {
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageRgb8(buffer) => {
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageRgba8(buffer) => {
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageBgr8(buffer) => {
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageBgra8(buffer) => {
                bytes8 = buffer.into_raw();
                &bytes8[..]
            }
            DynamicImage::ImageLuma16(buffer) => {
                bytes16 = buffer.into_raw();
                bytemuck::cast_slice(&bytes16[..])
            }
            DynamicImage::ImageLumaA16(buffer) => {
                bytes16 = buffer.into_raw();
                bytemuck::cast_slice(&bytes16[..])
            }
            DynamicImage::ImageRgb16(buffer) => {
                bytes16 = buffer.into_raw();
                bytemuck::cast_slice(&bytes16[..])
            }
            DynamicImage::ImageRgba16(buffer) => {
                bytes16 = buffer.into_raw();
                bytemuck::cast_slice(&bytes16[..])
            }
        };
        ctx.create_image_static(
            ImageInfo {
                extent: ImageExtent::D2 {
                    width: w,
                    height: h,
                },
                format,
                levels: 1,
                layers: 1,
                samples: Samples1,
                usage: ImageUsage::SAMPLED
                    | ImageUsage::STORAGE
                    | ImageUsage::TRANSFER_SRC,
            },
            &bytes,
        )
        .map(Texture)
    }
}

/// Quasi-format that tries to guess image format.
#[derive(Debug)]
pub struct GuessImageFormat;

impl<K> Format<Texture, K> for GuessImageFormat {
    type DecodeFuture = Ready<Result<DynamicImage, ImageError>>;
    type Error = ImageError;

    fn decode(self, bytes: Vec<u8>, _: &Cache<K>) -> Self::DecodeFuture {
        ready(load_from_memory(&bytes))
    }
}

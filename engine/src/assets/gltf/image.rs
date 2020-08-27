use {
    super::{GltfLoadingError, GltfRepr},
    crate::{assets::image_view_from_dyn_image, renderer::Context},
    illume::*,
};

pub fn load_gltf_image(
    repr: &GltfRepr,
    image: gltf::Image,
    ctx: &mut Context,
) -> Result<ImageView, GltfLoadingError> {
    match image.source() {
        gltf::image::Source::View { view, .. } => {
            let view_source = match view.buffer().source() {
                gltf::buffer::Source::Bin => repr.gltf.blob.as_deref(),
                gltf::buffer::Source::Uri(uri) => {
                    repr.buffers.get(uri).map(|b| &**b)
                }
            };

            let source_bytes =
                view_source.ok_or(GltfLoadingError::MissingSource)?;

            if source_bytes.len() < view.offset() + view.length() {
                return Err(GltfLoadingError::ViewOutOfBound);
            }

            let view_bytes = &source_bytes[view.offset()..][..view.length()];
            let dyn_image = image::load_from_memory(view_bytes)?;
            match image_view_from_dyn_image(&dyn_image, ctx) {
                Ok(view) => Ok(view),
                Err(CreateImageError::OutOfMemory { source }) => {
                    Err(GltfLoadingError::OutOfMemory { source })
                }
                Err(CreateImageError::Unsupported { info }) => {
                    Err(GltfLoadingError::UnsupportedImage { info })
                }
            }
        }
        gltf::image::Source::Uri { uri, .. } => Ok(repr.images[uri].clone()),
    }
}

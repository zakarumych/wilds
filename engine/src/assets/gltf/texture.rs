use {
    super::GltfLoadingError,
    crate::renderer::{Context, Texture},
    illume::{ImageView, Sampler, SamplerInfo},
};

pub fn load_gltf_texture(
    texture: gltf::Texture,
    views: &[ImageView],
    samplers: &[Sampler],
    default_sampler: &mut Option<Sampler>,
    ctx: &mut Context,
) -> Result<Texture, GltfLoadingError> {
    let image = views[texture.source().index()].clone();
    let sampler = match texture.sampler().index() {
        Some(index) => samplers[index].clone(),
        None => match default_sampler {
            Some(default_sampler) => default_sampler.clone(),
            None => {
                let sampler = ctx.create_sampler(SamplerInfo::default())?;
                *default_sampler = Some(sampler.clone());
                sampler
            }
        },
    };
    Ok(Texture { image, sampler })
}

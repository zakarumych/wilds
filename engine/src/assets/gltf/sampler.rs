use {
    crate::renderer::Context,
    gltf::texture::{MagFilter, MinFilter, WrappingMode},
    illume::*,
};

pub fn load_gltf_sampler(
    sampler: gltf::texture::Sampler,
    ctx: &mut Context,
) -> Result<Sampler, OutOfMemory> {
    ctx.create_sampler(SamplerInfo {
        mag_filter: match sampler.mag_filter() {
            None | Some(MagFilter::Nearest) => Filter::Nearest,
            Some(MagFilter::Linear) => Filter::Linear,
        },
        min_filter: match sampler.min_filter() {
            None
            | Some(MinFilter::Nearest)
            | Some(MinFilter::NearestMipmapNearest)
            | Some(MinFilter::NearestMipmapLinear) => Filter::Nearest,
            _ => Filter::Linear,
        },
        mipmap_mode: match sampler.min_filter() {
            None
            | Some(MinFilter::Nearest)
            | Some(MinFilter::Linear)
            | Some(MinFilter::NearestMipmapNearest)
            | Some(MinFilter::LinearMipmapNearest) => MipmapMode::Nearest,
            _ => MipmapMode::Linear,
        },
        address_mode_u: match sampler.wrap_s() {
            WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
            WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
            WrappingMode::Repeat => SamplerAddressMode::Repeat,
        },
        address_mode_v: match sampler.wrap_t() {
            WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
            WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
            WrappingMode::Repeat => SamplerAddressMode::Repeat,
        },
        address_mode_w: SamplerAddressMode::Repeat,
        mip_lod_bias: 0.0.into(),
        max_anisotropy: None,
        compare_op: None,
        min_lod: 0.0.into(),
        max_lod: 100.0.into(),
        border_color: BorderColor::FloatTransparentBlack,
        unnormalized_coordinates: false,
    })
}

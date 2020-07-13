use {
    illume::{ImageView, Sampler},
    ordered_float::OrderedFloat,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Texture {
    /// Image view of the loaded texture.
    pub image: ImageView,

    /// Sampler associated with the texture image.
    pub sampler: Sampler,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Material {
    pub albedo: Option<Texture>,
    pub albedo_factor: [OrderedFloat<f32>; 4],
    pub normal: Option<Texture>,
    pub normal_factor: OrderedFloat<f32>, /* normalInTangentSpace =
                                           * vec3(sampledNormal.xy
                                           * * normalScale,
                                           * sampledNormal.z) */
}

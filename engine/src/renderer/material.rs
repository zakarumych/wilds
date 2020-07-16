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

impl Material {
    pub fn color(rgba: [f32; 4]) -> Self {
        let [r, g, b, a] = rgba;
        Material {
            albedo: None,
            albedo_factor: [r.into(), g.into(), b.into(), a.into()],
            normal: None,
            normal_factor: 0.0.into(),
        }
    }
}

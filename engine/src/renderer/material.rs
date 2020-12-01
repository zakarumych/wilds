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
    pub metallic_roughness: Option<Texture>,
    pub metallic_factor: OrderedFloat<f32>,
    pub roughness_factor: OrderedFloat<f32>,
    pub emissive: Option<Texture>,
    pub emissive_factor: [OrderedFloat<f32>; 3],
    pub normal: Option<Texture>,
    pub normal_factor: OrderedFloat<f32>, /* normal_in_tangent_space =
                                           * vec3(sampled_normal.xy
                                           * * normal_factor,
                                           * sampled_normal.z) */
}

impl Default for Material {
    fn default() -> Self {
        Material::new()
    }
}

impl Material {
    pub const fn new() -> Material {
        Material {
            albedo: None,
            albedo_factor: [OrderedFloat(1.0); 4],
            metallic_roughness: None,
            metallic_factor: OrderedFloat(1.0),
            roughness_factor: OrderedFloat(1.0),
            emissive: None,
            emissive_factor: [OrderedFloat(0.0); 3],
            normal: None,
            normal_factor: OrderedFloat(1.0),
        }
    }

    pub fn color(rgba: [f32; 4]) -> Self {
        let [r, g, b, a] = rgba;
        Material {
            albedo_factor: [r.into(), g.into(), b.into(), a.into()],
            ..Material::new()
        }
    }
}

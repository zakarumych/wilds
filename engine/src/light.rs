use ultraviolet::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub radiance: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub radiance: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct SkyLight {
    pub radiance: [f32; 3],
}

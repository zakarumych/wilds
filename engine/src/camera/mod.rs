pub mod following;
pub mod free;

use nalgebra as na;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Camera {
    Perspective(na::Perspective3<f32>),
    Orthographic(na::Orthographic3<f32>),
    Matrix(na::Projective3<f32>),
}

impl Camera {
    pub fn projection(&self) -> na::Projective3<f32> {
        match *self {
            Self::Perspective(perspective) => perspective.to_projective(),
            Self::Orthographic(orthographic) => orthographic.to_projective(),
            Self::Matrix(mat) => mat,
        }
    }
}

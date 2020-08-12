pub mod following;
pub mod free;

use ultraviolet::{
    projection::{orthographic_vk, perspective_vk},
    Mat4,
};

#[derive(Clone, Copy, Debug)]
pub enum Camera {
    Perspective {
        vertical_fov: f32,
        aspect_ratio: f32,
        z_near: f32,
        z_far: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
    Matrix(Mat4),
}

impl Camera {
    pub fn projection(&self) -> Mat4 {
        match *self {
            Self::Perspective {
                vertical_fov,
                aspect_ratio,
                z_near,
                z_far,
            } => perspective_vk(vertical_fov, aspect_ratio, z_near, z_far),

            Self::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => orthographic_vk(left, right, bottom, top, near, far),

            Self::Matrix(mat) => mat,
        }
    }
}

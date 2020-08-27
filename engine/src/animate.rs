use {bumpalo::Bump, nalgebra as na, hecs::Entity};

/// Tree-like structure of joints.
#[derive(Debug)]
pub struct Skeleton {
    pub joints: Box<[Entity]>,
}

impl Skeleton {
    pub fn joints(&self) -> &[Entity] {
        &self.joints
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Joint {
    pub inverse_binding_matrix: na::Matrix4<f32>,
}

#[derive(Debug)]
pub struct Pose {
    pub matrices: Box<[na::Matrix4<f32>]>,
}

impl Pose {
    pub fn identity(size: usize) -> Pose {
        Pose {
            matrices: (0..size).map(|_| na::Matrix4::identity()).collect(),
        }
    }

    pub fn matrices(&self) -> &[na::Matrix4<f32>] {
        &self.matrices
    }
}

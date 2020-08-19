use {
    bumpalo::{Bump},
    ultraviolet::{Mat4, Rotor3, Vec3, Isometry3},
};

/// Tree-like structure of bones/joints.
pub struct Skeleton {
    bones: Box<[Bone]>,
}

impl Skeleton {
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }
}

pub struct Bone {
    parent: Option<usize>,
}

impl Bone {
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
}

pub struct Pose {
    isometries: Box<[Isometry3]>,
}

impl Pose {
    pub fn isometries(&self) -> &[Isometry3] {
        &self.isometries
    }
}

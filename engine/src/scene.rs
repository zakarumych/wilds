use {
    crate::{
        debug::EntityRefDisplay as _,
        engine::{System, SystemContext},
    },
    bumpalo::{collections::Vec as BVec, Bump},
    fastbitset::BumpBitSet,
    hecs::{Entity, EntityRef, World},
    nalgebra as na,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local3 {
    pub parent: Entity,
    pub iso: na::Isometry3<f32>,
    pub scale: na::Vector3<f32>,
}

impl Local3 {
    pub fn identity(parent: Entity) -> Self {
        Local3 {
            parent,
            iso: na::Isometry3::identity(),
            scale: na::Vector3::new(1.0, 1.0, 1.0),
        }
    }

    pub fn from_iso(parent: Entity, iso: na::Isometry3<f32>) -> Self {
        Local3 {
            parent,
            iso,
            scale: na::Vector3::new(1.0, 1.0, 1.0),
        }
    }

    pub fn from_translation(parent: Entity, tr: na::Translation3<f32>) -> Self {
        Local3 {
            parent,
            iso: na::Isometry3::from_parts(tr, na::UnitQuaternion::identity()),
            scale: na::Vector3::new(1.0, 1.0, 1.0),
        }
    }

    pub fn from_rotation(parent: Entity, rot: na::UnitQuaternion<f32>) -> Self {
        Local3 {
            parent,
            iso: na::Isometry3::from_parts(
                na::Translation3::new(0., 0., 0.),
                rot,
            ),
            scale: na::Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Global3 {
    pub iso: na::Isometry3<f32>,
    pub skew: na::Matrix3<f32>,
}

impl Global3 {
    pub fn identity() -> Self {
        Global3 {
            iso: na::Isometry3::identity(),
            skew: na::Matrix3::identity(),
        }
    }

    pub fn from_iso(iso: na::Isometry3<f32>) -> Self {
        Global3 {
            iso,
            skew: na::Matrix3::identity(),
        }
    }

    pub fn from_scale(scale: f32) -> Self {
        Global3 {
            iso: na::Isometry3::identity(),
            skew: na::Matrix3::from_diagonal(&na::Vector3::new(
                scale, scale, scale,
            )),
        }
    }

    pub fn from_nonuniform_scale(scale: na::Vector3<f32>) -> Self {
        Global3 {
            iso: na::Isometry3::identity(),
            skew: na::Matrix3::from_diagonal(&scale),
        }
    }

    pub fn append_iso_scale(
        &self,
        iso: &na::Isometry3<f32>,
        scale: &na::Vector3<f32>,
    ) -> Self {
        let total = self.to_homogeneous()
            * iso.to_homogeneous()
            * na::Matrix4::new_nonuniform_scaling(&scale);
        let rotation = self.iso.rotation * iso.rotation;
        let inv_rotation = rotation.inverse().to_rotation_matrix();
        let translation = total.column(3).xyz();
        let rotskew = total.remove_column(3).remove_row(3);
        let skew = inv_rotation * rotskew;

        Global3 {
            iso: na::Isometry3 {
                translation: na::Translation3 {
                    vector: translation,
                },
                rotation,
            },
            skew,
        }
    }

    pub fn append_local(&self, local: &Local3) -> Self {
        self.append_iso_scale(&local.iso, &local.scale)
    }

    pub fn append_global(&self, global: &Global3) -> Self {
        let total = self.to_homogeneous() * global.to_homogeneous();
        let rotation = self.iso.rotation * global.iso.rotation;
        let inv_rotation = rotation.inverse().to_rotation_matrix();
        let translation = total.column(3).xyz();
        let rotskew = total.remove_column(3).remove_row(3);
        let skew = inv_rotation * rotskew;

        Global3 {
            iso: na::Isometry3 {
                translation: na::Translation3 {
                    vector: translation,
                },
                rotation,
            },
            skew,
        }
    }

    pub fn to_homogeneous(&self) -> na::Matrix4<f32> {
        self.iso.to_homogeneous() * self.skew.to_homogeneous()
    }
}

pub struct SceneSystem;

impl System for SceneSystem {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let mut updated = BumpBitSet::new();
        let mut despawn = BVec::new_in(ctx.bump);

        for (entity, local) in
            ctx.world.query::<&Local3>().with::<Global3>().iter()
        {
            update_global(
                entity,
                ctx.world.entity(entity).unwrap(),
                local,
                ctx.world,
                ctx.bump,
                &mut updated,
                &mut despawn,
            );
        }

        // Despawn entities whose parents are despawned.
        for entity in despawn {
            let _ = ctx.world.despawn(entity);
        }
    }
}

fn update_global<'a>(
    entity: Entity,
    entity_ref: EntityRef<'a>,
    local: &Local3,
    world: &'a World,
    bump: &'a Bump,
    updated: &mut BumpBitSet<'a>,
    despawn: &mut BVec<'a, Entity>,
) -> Option<hecs::RefMut<'a, Global3>> {
    let parent_ref = match world.entity(local.parent) {
        Ok(parent_ref) => parent_ref,
        Err(hecs::NoSuchEntity) => {
            despawn.push(entity);
            return None;
        }
    };
    let parent_local = parent_ref.get::<Local3>();

    match parent_local {
        None => {
            // Parent has no parent node.
            match parent_ref.get::<Global3>() {
                Some(parent_global_ref) => {
                    // Parent is root node.
                    let global = parent_global_ref.append_local(local);
                    drop(parent_global_ref);

                    let mut global_ref =
                        entity_ref.get_mut::<Global3>().unwrap();
                    *global_ref = global;

                    Some(global_ref)
                }
                None => {
                    // Parent is not in hierarchy.
                    tracing::warn!(
                        "Entity's ({}) parent is not in scene and shall be despawned", entity_ref.display(entity)
                    );
                    despawn.push(entity);
                    None
                }
            }
        }
        Some(parent_local) => {
            let parent_global = if !updated.set(local.parent.id(), bump) {
                update_global(
                    local.parent,
                    parent_ref,
                    &parent_local,
                    world,
                    bump,
                    updated,
                    despawn,
                )
            } else {
                parent_ref.get_mut::<Global3>()
            };

            match parent_global {
                Some(parent_global) => {
                    let global = parent_global.append_local(local);
                    drop(parent_global);

                    let mut global_ref =
                        entity_ref.get_mut::<Global3>().unwrap();
                    *global_ref = global;
                    Some(global_ref)
                }
                None => {
                    despawn.push(entity);
                    None
                }
            }
        }
    }
}

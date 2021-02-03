use {
    crate::engine::{System, SystemContext},
    bumpalo::collections::Vec as BVec,
    nalgebra as na,
    rapier3d::{
        dynamics::{
            IntegrationParameters, JointSet, RigidBody, RigidBodyHandle,
            RigidBodySet,
        },
        geometry::{
            BroadPhase, Collider, ColliderHandle, ColliderSet, NarrowPhase,
        },
        pipeline::PhysicsPipeline,
    },
};

pub struct Physics {
    pipeline: PhysicsPipeline,
    integration_parameters: IntegrationParameters,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
}

impl Physics {
    pub fn new() -> Self {
        Physics {
            pipeline: PhysicsPipeline::new(),
            integration_parameters: IntegrationParameters::default(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
        }
    }
}

pub struct PhysicsData {
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub joints: JointSet,
}

impl PhysicsData {
    pub fn new() -> Self {
        PhysicsData {
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            joints: JointSet::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Constants {
    pub gravity: na::Vector3<f32>,
}

impl Constants {
    pub fn new() -> Self {
        Constants {
            gravity: na::Vector3::new(0.0, 0.0, 0.0),
        }
    }
}

impl Default for Constants {
    fn default() -> Self {
        Self::new()
    }
}

impl System for Physics {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let constants = *ctx
            .resources
            .entry::<Constants>()
            .or_insert_with(Constants::new);

        let sets = ctx
            .resources
            .entry::<PhysicsData>()
            .or_insert_with(PhysicsData::new);

        self.pipeline.step(
            &constants.gravity,
            &self.integration_parameters,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut sets.bodies,
            &mut sets.colliders,
            &mut sets.joints,
            None,
            None,
            &(),
        );
    }
}

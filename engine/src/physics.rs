use {
    crate::{
        engine::{System, SystemContext},
        scene::Global3,
    },
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

pub use rapier3d::*;

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

impl Default for PhysicsData {
    fn default() -> Self {
        PhysicsData::new()
    }
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
    fn name(&self) -> &str {
        "Physics"
    }

    fn run(&mut self, ctx: SystemContext<'_>) {
        let constants = *ctx.resources.get_or_else(Constants::new);

        // self.integration_parameters.dt = ctx.clocks.delta.as_secs_f32();

        let sets = ctx.resources.get_or_else(PhysicsData::new);

        for (e, (global, body)) in
            ctx.world.query::<(&Global3, &RigidBodyHandle)>().iter()
        {
            let body = sets.bodies.get_mut(*body).unwrap();
            if *body.position() != global.iso {
                body.set_position(global.iso, true);
            }
        }

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

        for (_, (global, body)) in
            ctx.world.query::<(&mut Global3, &RigidBodyHandle)>().iter()
        {
            let body = sets.bodies.get_mut(*body).unwrap();
            global.iso = *body.position();
        }
    }
}

use {
    crate::{
        engine::{System, SystemContext},
        scene::Global3,
    },
    hecs::{Entity, World},
    nalgebra as na,
    ncollide3d::shape::ShapeHandle,
    nphysics3d::{
        force_generator::DefaultForceGeneratorSet,
        joint::DefaultJointConstraintSet,
        object::{Body, BodySet, DefaultColliderHandle, DefaultColliderSet},
        world::{GeometricalWorld, MechanicalWorld},
    },
    parking_lot::Mutex,
    smallvec::{smallvec, SmallVec},
};

pub use nphysics3d::object::{
    BodyPartHandle, BodyStatus, Collider, ColliderDesc, RigidBody,
    RigidBodyDesc,
};

// FIXME: All `Physics` instances share colliders set.
lazy_static::lazy_static! {
    pub static ref COLLIDER_SET: Mutex<DefaultColliderSet<f32, Entity>> = Mutex::new(DefaultColliderSet::new());
}

#[derive(Clone, Copy, Debug)]
pub struct Constants {
    pub time_factor: f32,
}

impl Constants {
    const fn new() -> Self {
        Constants { time_factor: 1.0 }
    }
}

impl Default for Constants {
    fn default() -> Self {
        Constants::new()
    }
}

pub struct Physics {
    geometrical: GeometricalWorld<f32, Entity, DefaultColliderHandle>,
    mechanical: MechanicalWorld<f32, Entity, DefaultColliderHandle>,
    // body_set: DefaultBodySet<f32>,
    // collider_set: DefaultColliderSet<f32>,
    joint_constraint_set: DefaultJointConstraintSet<f32, Entity>,
    force_generator_set: DefaultForceGeneratorSet<f32, Entity>,
}

pub struct Colliders {
    array: SmallVec<[(ColliderDesc<f32>, usize); 1]>,
}

impl Colliders {
    pub fn new(collider: ColliderDesc<f32>) -> Self {
        Colliders {
            array: smallvec![(collider, 0)],
        }
    }

    pub fn new_part(collider: ColliderDesc<f32>, part: usize) -> Self {
        Colliders {
            array: smallvec![(collider, part)],
        }
    }
}

impl From<ColliderDesc<f32>> for Colliders {
    fn from(desc: ColliderDesc<f32>) -> Self {
        Colliders::new(desc)
    }
}

impl From<ShapeHandle<f32>> for Colliders {
    fn from(shape: ShapeHandle<f32>) -> Self {
        Colliders::new(ColliderDesc::new(shape))
    }
}

struct AttachedColliders {
    array: SmallVec<[DefaultColliderHandle; 1]>,
}

impl Drop for AttachedColliders {
    fn drop(&mut self) {
        let mut lock = COLLIDER_SET.lock();
        for handle in self.array.drain(..) {
            lock.remove(handle);
        }
    }
}

impl Physics {
    pub fn new() -> Self {
        let geometrical = GeometricalWorld::new();
        let mechanical = MechanicalWorld::new(na::Vector3::y() * -100.0);
        // let body_set = DefaultBodySet::new();
        // let collider_set = DefaultColliderSet::new();
        let joint_constraint_set = DefaultJointConstraintSet::new();
        let force_generator_set = DefaultForceGeneratorSet::new();

        Physics {
            geometrical,
            mechanical,
            // body_set,
            // collider_set,
            joint_constraint_set,
            force_generator_set,
        }
    }
}

impl System for Physics {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let world = ctx.world;

        const DEFAULT_CONSTANTS: Constants = Constants::new();
        let constants = ctx
            .resources
            .get::<Constants>()
            .unwrap_or(&DEFAULT_CONSTANTS);

        let delta = ctx.clocks.delta.as_secs_f32() * constants.time_factor;

        let mut lock = None;

        let attached: Vec<_> = world
            .query::<&Colliders>()
            .without::<AttachedColliders>()
            .iter()
            .map(|(entity, colliders)| {
                let array = colliders
                    .array
                    .iter()
                    .map(|(desc, part)| {
                        let lock =
                            lock.get_or_insert_with(|| COLLIDER_SET.lock());
                        let collider =
                            desc.build(BodyPartHandle(entity, *part));
                        lock.insert(collider)
                    })
                    .collect();
                (entity, AttachedColliders { array })
            })
            .collect();

        for (entity, attached) in attached {
            world.insert_one(entity, attached).unwrap();
        }

        for (_, (global, body)) in
            world.query::<(&Global3, &mut RigidBody<f32>)>().iter()
        {
            // FIXME: Update position only if changed.
            body.set_position(global.iso);
        }

        let lock = lock.get_or_insert_with(|| COLLIDER_SET.lock());

        self.mechanical.maintain(
            &mut self.geometrical,
            WorldBodySet::cast(world),
            &mut **lock,
            &mut self.joint_constraint_set,
        );

        self.mechanical.set_timestep(delta.min(0.01666666666666));
        self.mechanical.step(
            &mut self.geometrical,
            WorldBodySet::cast(world),
            &mut **lock,
            &mut self.joint_constraint_set,
            &mut self.force_generator_set,
        );

        for (_, (global, body)) in
            world.query::<(&mut Global3, &RigidBody<f32>)>().iter()
        {
            // FIXME: Update position only if changed.
            global.iso = *body.position();
        }
    }
}

#[repr(transparent)]
struct WorldBodySet {
    world: World,
}

impl WorldBodySet {
    fn cast(world: &mut World) -> &mut Self {
        unsafe { &mut *(world as *mut _ as *mut _) }
    }
}

impl BodySet<f32> for WorldBodySet {
    type Handle = Entity;

    fn get(&self, entity: Entity) -> Option<&dyn Body<f32>> {
        match unsafe { self.world.get_unchecked::<RigidBody<f32>>(entity) } {
            Ok(body) => Some(body),
            _ => None,
        }
    }

    fn get_mut(&mut self, entity: Entity) -> Option<&mut dyn Body<f32>> {
        match unsafe { self.world.get_unchecked_mut::<RigidBody<f32>>(entity) }
        {
            Ok(body) => Some(body),
            _ => None,
        }
    }

    fn contains(&self, entity: Entity) -> bool {
        self.world.contains(entity)
    }

    fn foreach(&self, f: &mut dyn FnMut(Entity, &dyn Body<f32>)) {
        for (e, b) in self.world.query::<&RigidBody<f32>>().iter() {
            f(e, b)
        }
    }

    fn foreach_mut(&mut self, f: &mut dyn FnMut(Entity, &mut dyn Body<f32>)) {
        for (e, b) in self.world.query::<&mut RigidBody<f32>>().iter() {
            f(e, b)
        }
    }

    fn pop_removal_event(&mut self) -> Option<Entity> {
        None
    }
}

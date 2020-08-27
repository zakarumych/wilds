use {
    crate::{
        clocks::ClockIndex,
        engine::{InputEvents, System, SystemContext},
        util::iso_to_nalgebra,
    },
    hecs::{Entity, QueryBorrow, QueryOne, World},
    nalgebra as na,
    ncollide3d::{
        bounding_volume::AABB,
        broad_phase::{
            BroadPhase as _, BroadPhaseInterferenceHandler,
            BroadPhaseProxyHandle, DBVTBroadPhase,
        },
        narrow_phase::{
            CollisionObjectGraphIndex, InteractionGraph, NarrowPhase,
        },
        pipeline::{
            glue::default_narrow_phase,
            object::{
                CollisionGroups, CollisionObjectRef, CollisionObjectSet,
                CollisionObjectUpdateFlags, GeometricQueryType,
            },
        },
        shape::{Shape, ShapeHandle},
    },
};

/// Body type defines how entity is handled by physics engine.
/// Entity should never change its body type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyType {
    /// Static bodies never moves.
    /// This means that once isometry, body and shape is given to entity
    /// physics system will remember position and will never update it.
    /// Moving this entity may result in inconsistency between different
    /// systems.
    ///
    /// Most objects are expected to be static.
    Static,

    /// Kinematic bodies can be collided with others, but their isometry is
    /// never changed by physics system.
    Kinematic,

    /// Dynamic bodies can be collided with other bodies and physics system
    /// will attempt to resolve contacts by moving dynamic bodies.
    ///
    /// Few objects are expected to be dynamic.
    Dynamic,
}

impl BodyType {
    fn collision_groups(&self) -> CollisionGroups {
        let cg = CollisionGroups::new().with_membership(&[*self as usize]);

        match self {
            BodyType::Dynamic => cg.with_whitelist(&[
                BodyType::Static as usize,
                BodyType::Kinematic as usize,
                BodyType::Dynamic as usize,
            ]),
            _ => cg.with_whitelist(&[BodyType::Dynamic as usize]),
        }
    }
}

pub struct Physics {
    broad_phase: DBVTBroadPhase<f32, AABB<f32>, Entity>,
    narrow_phase: NarrowPhase<f32, Entity>,
    interactions: InteractionGraph<f32, Entity>,
    gravity: Vec3,
}

impl Physics {
    pub fn new() -> Self {
        let broad_phase = DBVTBroadPhase::new(0.1);
        let narrow_phase = default_narrow_phase();
        let interactions = InteractionGraph::new();

        Physics {
            broad_phase,
            narrow_phase,
            interactions,
            gravity: Vec3::unit_y() * -0.03,
        }
    }
}

pub struct Physical {
    pub shape: ShapeHandle<f32>,
    pub body_type: BodyType,
    pub velocity: Vec3,
}

struct CollisionObject {
    local_aabb: AABB<f32>,
    shape: ShapeHandle<f32>,
    proxy: BroadPhaseProxyHandle,
    graph_index: CollisionObjectGraphIndex,
    iso: na::Isometry3<f32>,
    groups: CollisionGroups,
    update_flags: CollisionObjectUpdateFlags,
}

impl CollisionObjectRef<f32> for CollisionObject {
    fn graph_index(&self) -> Option<CollisionObjectGraphIndex> {
        Some(self.graph_index)
    }

    fn proxy_handle(&self) -> Option<BroadPhaseProxyHandle> {
        Some(self.proxy)
    }

    fn position(&self) -> &na::Isometry3<f32> {
        &self.iso
    }

    fn predicted_position(&self) -> Option<&na::Isometry3<f32>> {
        None
    }

    fn shape(&self) -> &dyn Shape<f32> {
        self.shape.as_ref()
    }

    fn collision_groups(&self) -> &CollisionGroups {
        &self.groups
    }

    fn query_type(&self) -> GeometricQueryType<f32> {
        GeometricQueryType::Contacts(0.1, 0.1)
    }

    fn update_flags(&self) -> CollisionObjectUpdateFlags {
        self.update_flags
    }
}

struct Animated;
struct Dynamic;

impl System for Physics {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let world = ctx.world;
        let delta = ctx.clocks.delta.as_secs_f32();

        // Remove internal components for entites that lost their `Physical`
        // component.
        let mut not_physical_anymore_query = world
            .query::<(Option<&Animated>, Option<&Dynamic>)>()
            .without::<Physical>()
            .with::<CollisionObject>();
        let to_remove = not_physical_anymore_query
            .iter()
            .map(|(entity, (animated, dynamic))| {
                (entity, animated.is_some(), dynamic.is_some())
            })
            .collect::<Vec<_>>();
        drop(not_physical_anymore_query);

        for (entity, animated, dynamic) in to_remove {
            match (animated, dynamic) {
                (true, true) => {
                    world
                        .remove::<(CollisionObject, Animated, Dynamic)>(entity)
                        .unwrap();
                }
                (true, false) => {
                    world
                        .remove::<(CollisionObject, Animated)>(entity)
                        .unwrap();
                }
                (false, false) => {
                    world.remove_one::<CollisionObject>(entity).unwrap();
                }
                (false, true) => {
                    tracing::warn!(
                        "Inanimated dynamic entity found. This is unexpected"
                    );
                    world.remove::<(CollisionObject, Dynamic)>(entity).unwrap();
                }
            }
        }

        {
            let mut dynamics_query =
                world.query::<&mut Physical>().with::<Dynamic>();

            for (_, physical) in dynamics_query.iter() {
                physical.velocity += self.gravity * delta;
            }
        }

        {
            let mut animated_query = world
                .query::<(&mut Isometry3, &mut Physical, &mut CollisionObject)>(
                )
                .with::<Animated>();

            for (_, (iso, physical, cobj)) in animated_query.iter() {
                if physical.velocity.mag_sq() > 0.0001 {
                    iso.append_translation(physical.velocity * delta);
                    cobj.iso = iso_to_nalgebra(&iso);
                    let aabb = cobj.local_aabb.transform_by(&cobj.iso);
                    self.broad_phase
                        .deferred_set_bounding_volume(cobj.proxy, aabb);
                    cobj.update_flags |=
                        CollisionObjectUpdateFlags::POSITION_CHANGED;
                }
            }
        }

        let mut new_entities_query = world
            .query::<(&Isometry3, &Physical)>()
            .without::<CollisionObject>();

        let new_entities = new_entities_query
            .iter()
            .map(|(entity, (iso, physical))| {
                let local_aabb = physical.shape.local_aabb();
                let aabb = local_aabb.transform_by(&iso_to_nalgebra(&iso));
                let proxy = self.broad_phase.create_proxy(aabb, entity);
                let graph_index = self.interactions.add_node(entity);
                let cobj = CollisionObject {
                    local_aabb,
                    shape: physical.shape.clone(),
                    proxy,
                    graph_index,
                    iso: iso_to_nalgebra(iso),
                    groups: physical.body_type.collision_groups(),
                    update_flags: CollisionObjectUpdateFlags::empty(),
                };
                (entity, cobj, physical.body_type)
            })
            .collect::<Vec<_>>();
        drop(new_entities_query);

        for (entity, detail, bt) in new_entities {
            match bt {
                BodyType::Static => world.insert_one(entity, detail),
                BodyType::Kinematic => world.insert(entity, (detail, Animated)),
                BodyType::Dynamic => {
                    world.insert(entity, (detail, Animated, Dynamic))
                }
            }
            .unwrap()
        }

        self.broad_phase
            .update(&mut WildsBroadPhaseInterferenceHandler {
                world,
                narrow_phase: &mut self.narrow_phase,
                interactions: &mut self.interactions,
            });

        self.narrow_phase.update(
            &mut self.interactions,
            WorldCollisionObjectSet::cast(world),
        );

        for event in self.narrow_phase.proximity_events() {
            eprintln!("{:#?}", event);
        }

        for event in self.narrow_phase.contact_events() {
            eprintln!("{:#?}", event);
        }

        self.narrow_phase.clear_events();

        for (l, r, _, m) in self.interactions.contact_pairs(false) {}
    }
}

#[repr(transparent)]
struct WorldCollisionObjectSet(World);

impl WorldCollisionObjectSet {
    fn cast(world: &mut World) -> &mut Self {
        unsafe { &mut *(world as *mut _ as *mut _) }
    }
}

impl CollisionObjectSet<f32> for WorldCollisionObjectSet {
    type CollisionObject = CollisionObject;
    type CollisionObjectHandle = Entity;

    fn collision_object(&self, entity: Entity) -> Option<&CollisionObject> {
        unsafe { self.0.get_unchecked::<CollisionObject>(entity) }.ok()
    }

    fn foreach(&self, mut f: impl FnMut(Entity, &CollisionObject)) {
        let mut query = self.0.query::<&CollisionObject>();

        for (entity, cobj) in query.iter() {
            f(entity, cobj);
        }
    }
}

struct WildsBroadPhaseInterferenceHandler<'a> {
    world: &'a mut World,
    narrow_phase: &'a mut NarrowPhase<f32, Entity>,
    interactions: &'a mut InteractionGraph<f32, Entity>,
}

impl BroadPhaseInterferenceHandler<Entity>
    for WildsBroadPhaseInterferenceHandler<'_>
{
    fn is_interference_allowed(&mut self, b1: &Entity, b2: &Entity) -> bool {
        let b1 = self.world.get::<Physical>(*b1);
        let b2 = self.world.get::<Physical>(*b2);

        match (b1, b2) {
            (Ok(b1), Ok(b2)) => {
                matches!((b1.body_type, b2.body_type), (BodyType::Dynamic, _)
| (_, BodyType::Dynamic))
            }
            _ => false,
        }
    }

    fn interference_started(&mut self, b1: &Entity, b2: &Entity) {
        self.narrow_phase.handle_interaction(
            self.interactions,
            WorldCollisionObjectSet::cast(&mut self.world),
            *b1,
            *b2,
            true,
        )
    }

    fn interference_stopped(&mut self, b1: &Entity, b2: &Entity) {
        self.narrow_phase.handle_interaction(
            &mut self.interactions,
            WorldCollisionObjectSet::cast(&mut self.world),
            *b1,
            *b2,
            false,
        )
    }
}

use {
    bytemuck::cast_slice,
    hecs::{Entity, World},
    nalgebra as na,
    std::sync::Arc,
    wilds::{
        assets::{Prefab, SyncAsset},
        engine::{System, SystemContext},
        physics::{
            dynamics::{RigidBodyBuilder, RigidBodyHandle},
            geometry::{Capsule, ColliderBuilder, SharedShape},
            PhysicsData,
        },
        renderer::{
            BufferUsage, Context, Material, Mesh, MeshData, Normal3d,
            OutOfMemory, Position3d, PositionNormalTangent3dUV,
            PrimitiveTopology, Renderable, Tangent3d, UV,
        },
        resources::Resources,
        scene::Global3,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pawn {
    dir: na::Vector3<f32>,
    force: na::Vector3<f32>,
}

impl Pawn {
    pub fn new() -> Self {
        Pawn {
            dir: na::Vector3::new(1.0, 0.0, 1.0),
            force: na::Vector3::default(),
        }
    }
}

#[derive(Clone)]
pub struct PawnAsset {
    pub mesh: Mesh,
    pub shape: Arc<Capsule>,
}

impl PawnAsset {
    pub fn new(
        radius: f32,
        height: f32,
        ctx: &mut Context,
    ) -> Result<Self, OutOfMemory> {
        let capsule = Capsule::new(
            na::Point3::new(0.0, radius, 0.0),
            na::Point3::new(0.0, height - radius, 0.0),
            radius,
        );

        let (vertices, indices) = capsule.to_trimesh(16, 16);
        let mut normals = vec![(na::Vector3::default(), 0); vertices.len()];

        for &[a, b, c] in &indices {
            let av = vertices[a as usize].coords;
            let bv = vertices[b as usize].coords;
            let cv = vertices[c as usize].coords;

            let n = (bv - av).cross(&(cv - av)).normalize();

            let (normal, weight) = &mut normals[a as usize];
            *normal += n;
            *weight += 1;

            let (normal, weight) = &mut normals[b as usize];
            *normal += n;
            *weight += 1;

            let (normal, weight) = &mut normals[c as usize];
            *normal += n;
            *weight += 1;
        }

        let normals = normals.iter().copied().map(|(n, w)| n / w as f32);

        let vertices: Vec<_> = Iterator::zip(vertices.iter().copied(), normals)
            .map(|(pos, norm)| PositionNormalTangent3dUV {
                position: Position3d(pos.coords.into()),
                normal: Normal3d(norm.into()),
                tangent: Tangent3d([1.0; 4]),
                uv: UV([0.0; 2]),
            })
            .collect();

        let usage = BufferUsage::STORAGE
            | BufferUsage::DEVICE_ADDRESS
            | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT;

        let mesh = MeshData::new(PrimitiveTopology::TriangleList)
            .with_binding(&vertices)
            .with_indices(cast_slice::<_, u32>(&*indices))
            .build(ctx, usage, usage)?;

        let shape = Arc::new(capsule);

        Ok(PawnAsset { mesh, shape })
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct PawnRepr {
    height: f32,
    diameter: f32,
}

impl SyncAsset for PawnAsset {
    type Context = Context;
    type Repr = PawnRepr;

    fn build(repr: PawnRepr, ctx: &mut Context) -> eyre::Result<Self> {
        PawnAsset::new(repr.diameter, repr.height, ctx).map_err(Into::into)
    }
}

impl Prefab for Pawn {
    type Asset = PawnAsset;

    fn spawn(
        asset: PawnAsset,
        world: &mut World,
        resources: &mut Resources,
        entity: Entity,
    ) {
        let sets = resources.get_or_else(PhysicsData::new);

        let body = sets.bodies.insert(
            RigidBodyBuilder::new_dynamic()
                // .restrict_rotations(false, true, false)
                .lock_rotations()
                .build(),
        );

        let collider = sets.colliders.insert(
            ColliderBuilder::new(SharedShape(asset.shape)).build(),
            body,
            &mut sets.bodies,
        );

        let _ = world.insert(
            entity,
            (
                Renderable {
                    mesh: asset.mesh,
                    material: Material::color([0.3, 1.0, 0.7, 1.0])
                        .with_metalness(1.0)
                        .with_roughness(0.1),
                    transform: None,
                },
                body,
                collider,
                Pawn::new(),
            ),
        );
    }
}

pub struct PawnSystem;

impl System for PawnSystem {
    fn name(&self) -> &str {
        "Pawn"
    }

    fn run(&mut self, ctx: SystemContext<'_>) {
        let dt = ctx.clocks.delta.as_secs_f32();
        let sets = ctx.resources.get_or_else(PhysicsData::new);
        let mut query =
            ctx.world.query::<(&RigidBodyHandle, &mut Pawn, &Global3)>();

        for (_, (&body, pawn, global)) in query.iter() {
            if global.iso.translation.vector.magnitude() > 50.0 {
                pawn.dir = (-global.iso.translation.vector).normalize() / 5.0;
            } else if rand::random::<f32>() > 0.8f32.powf(dt) {
                pawn.dir = na::Vector3::new(
                    rand::random::<f32>() - 0.5,
                    rand::random::<f32>() - 0.5,
                    rand::random::<f32>() - 0.5,
                )
                .normalize()
                    / 5.0;
            }

            let body = sets.bodies.get_mut(body).unwrap();
            let linvel = body.linvel();
            pawn.force += (pawn.dir - *linvel)
                .component_mul(&na::Vector3::new(1.0, 0.0, 1.0));

            if pawn.force.magnitude() > 1.0 {
                pawn.force /= pawn.force.magnitude();
            }

            body.apply_force(pawn.force, true)
        }
    }
}

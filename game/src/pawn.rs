use {
    bytemuck::cast_slice,
    hecs::{Entity, World},
    nalgebra as na,
    std::sync::Arc,
    wilds::{
        assets::{Prefab, SyncAsset},
        physics::{
            dynamics::RigidBodyBuilder,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pawn;

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
            na::Point3::new(0.0, 0.0, radius),
            na::Point3::new(0.0, 0.0, height - radius),
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
    type Error = OutOfMemory;
    type Repr = PawnRepr;

    fn build(repr: PawnRepr, ctx: &mut Context) -> Result<Self, OutOfMemory> {
        PawnAsset::new(repr.diameter, repr.height, ctx)
    }
}

impl Prefab for Pawn {
    type Asset = PawnAsset;
    type Info = na::Isometry3<f32>;

    fn spawn(
        asset: PawnAsset,
        iso: na::Isometry3<f32>,
        world: &mut World,
        resources: &mut Resources,
        entity: Entity,
    ) {
        let sets = resources.get_or_else(PhysicsData::new);

        let body = sets
            .bodies
            .insert(RigidBodyBuilder::new_dynamic().lock_rotations().build());

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
                    material: Material::color([0.7, 0.5, 0.3, 1.0]),
                    // transform: None,
                },
                body,
                collider,
                Global3::from_iso(iso),
                Pawn,
            ),
        );
    }
}

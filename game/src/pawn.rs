use {
    bytemuck::cast_slice,
    eyre::Report,
    goods::SyncAsset,
    hecs::{Entity, World},
    nalgebra as na,
    ncollide3d::{
        math::Point,
        procedural::{capsule, IndexBuffer},
        shape::{Capsule, ShapeHandle},
    },
    std::sync::Arc,
    ultraviolet::{Isometry3, Vec3},
    wilds::{
        assets::Prefab,
        physics::{
            BodyPartHandle, ColliderDesc, Colliders, Physics, RigidBodyDesc,
        },
        renderer::{
            BindingData, BufferUsage, Context, Material, Mesh, MeshData,
            Normal3d, OutOfMemory, Position3d, PositionNormalTangent3dUV,
            PrimitiveTopology, Renderable, Tangent3d, UV,
        },
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pawn;

#[derive(Clone)]
pub struct PawnAsset {
    pub mesh: Mesh,
    pub shape: Arc<Capsule<f32>>,
}

impl PawnAsset {
    pub fn new(
        diameter: f32,
        height: f32,
        ctx: &mut Context,
    ) -> Result<Self, OutOfMemory> {
        let trimesh = capsule(&diameter, &height, 16, 16);

        assert!(trimesh.has_normals());
        let normals = trimesh.normals.as_ref().unwrap();

        let indices = match trimesh.indices {
            IndexBuffer::Unified(indices) => indices,
            _ => panic!("Split indices are unsupported"),
        };

        let vertices: Vec<_> =
            Iterator::zip(trimesh.coords.iter(), normals.iter())
                .map(|(&pos, &norm)| PositionNormalTangent3dUV {
                    position: Position3d(pos.coords.into()),
                    normal: Normal3d(norm.into()),
                    tangent: Tangent3d([1.0; 4]),
                    uv: UV([0.0; 2]),
                })
                .collect();

        let usage = BufferUsage::STORAGE
            | BufferUsage::SHADER_DEVICE_ADDRESS
            | BufferUsage::RAY_TRACING;

        let mesh = MeshData::new(PrimitiveTopology::TriangleList)
            .with_binding(&vertices)
            .with_indices(cast_slice::<_, u32>(unsafe {
                // `Point<u32>` (alias to `nalgebra::Point<u32, U3>`)
                // and `[u32; 3]` have same repr.
                std::mem::transmute::<&[Point<u32>], &[[u32; 3]]>(&*indices)
            }))
            .build(ctx, usage, usage)?;

        let capsule = Capsule::new((height + diameter) / 2.0, diameter / 2.0);
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

impl Prefab for PawnAsset {
    type Info = Isometry3;

    fn spawn(self, iso: Isometry3, world: &mut World, entity: Entity) {
        let body = RigidBodyDesc::<f32>::new()
            .kinematic_rotations(na::Vector3::new(true, true, true))
            .build();

        let _ = world.insert(
            entity,
            (
                Renderable {
                    mesh: self.mesh,
                    material: Material::color([0.7, 0.5, 0.3, 1.0]),
                    transform: None,
                },
                body,
                Colliders::from(
                    ColliderDesc::new(ShapeHandle::from_arc(self.shape))
                        .density(1.0)
                        .margin(0.3),
                ),
                iso,
                Pawn,
            ),
        );
    }
}

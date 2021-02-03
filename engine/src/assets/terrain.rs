use {
    super::{
        append_key,
        material::{MaterialInfo, MaterialRepr},
        ready, Asset, AssetKey, Assets, Format, Prefab,
    },
    crate::{
        physics::PhysicsData,
        renderer::{
            Context, Material, Mesh, MeshBuilder, Normal3d, Position3d,
            PositionNormalTangent3dUV, Renderable, Tangent3d, VertexType as _,
            UV,
        },
        scene::Global3,
    },
    futures::future::BoxFuture,
    hecs::{Entity, World},
    illume::{
        BufferInfo, BufferUsage, IndexType, OutOfMemory, PrimitiveTopology,
    },
    image::{
        load_from_memory, DynamicImage, GenericImageView, ImageError, Pixel,
    },
    nalgebra as na,
    num_traits::{bounds::Bounded, cast::ToPrimitive},
    parry3d::shape::HeightField,
    rapier3d::{
        dynamics::{RigidBodyBuilder, RigidBodySet},
        geometry::{ColliderBuilder, ColliderSet, SharedShape},
    },
    std::{convert::TryFrom as _, sync::Arc},
    type_map::TypeMap,
};

pub fn create_terrain_shape(
    width: u32,
    depth: u32,
    height: impl Fn(u32, u32) -> f32,
) -> HeightField {
    let mut matrix: na::DMatrix<f32> = na::DMatrix::zeros_generic(
        na::Dynamic::new(depth as usize),
        na::Dynamic::new(width as usize),
    );

    for x in 0..width {
        for y in 0..depth {
            matrix[(y as usize, x as usize)] = height(x, y);
        }
    }

    HeightField::new(matrix, na::Vector3::new(width as f32, 1.0, depth as f32))
}

pub fn create_terrain_mesh(
    width: u32,
    depth: u32,
    height: impl Fn(u32, u32) -> f32,
    buffer_usage: BufferUsage,
    ctx: &mut Context,
) -> Result<Mesh, OutOfMemory> {
    if width.checked_mul(depth).is_none() {
        return Err(OutOfMemory);
    }

    let vertex_count = width * depth;

    let vertex_total_size = usize::try_from(vertex_count)
        .ok()
        .and_then(|count| {
            std::alloc::Layout::array::<PositionNormalTangent3dUV>(count).ok()
        })
        .expect("Terrain is too large")
        .size();

    let index_count = (width.saturating_sub(1) * depth.saturating_sub(1))
        .checked_mul(6)
        .expect("Terrain is too large");

    let index_total_size = usize::try_from(index_count)
        .ok()
        .and_then(|count| std::alloc::Layout::array::<u32>(count).ok())
        .expect("Terrain is too large")
        .size();

    let total_size = vertex_total_size
        .checked_add(index_total_size)
        .expect("Terrain is too large");

    u64::try_from(total_size).expect("Terrain is too large");

    let mut data: Vec<u8> = Vec::with_capacity(total_size);

    let xoff = width as f32 * 0.5;
    let zoff = depth as f32 * 0.5;

    for z in 0..depth {
        for x in 0..width {
            let h = height(x, z);
            let h_n = if z == depth - 1 { h } else { height(x, z + 1) };
            let h_s = if z == 0 { h } else { height(x, z - 1) };
            let h_w = if x == 0 { h } else { height(x - 1, z) };
            let h_e = if x == width - 1 { h } else { height(x + 1, z) };

            let shift_n = na::Vector3::from([0.0, h_n - h, 1.0]);
            let shift_s = na::Vector3::from([0.0, h_s - h, -1.0]);
            let shift_w = na::Vector3::from([-1.0, h_w - h, 0.0]);
            let shift_e = na::Vector3::from([1.0, h_e - h, 0.0]);

            let normal = (shift_n.cross(&shift_e)
                + shift_e.cross(&shift_s)
                + shift_s.cross(&shift_w)
                + shift_w.cross(&shift_n))
            .normalize();

            let tangent = Tangent3d([1.0, 0.0, 0.0, 1.0]);

            let xf = x as f32 - xoff;
            let zf = z as f32 - zoff;

            let u = x as f32;
            let v = z as f32;

            data.extend_from_slice(bytemuck::cast_slice(&[
                PositionNormalTangent3dUV {
                    position: Position3d([xf, h, zf]),
                    normal: Normal3d(normal.into()),
                    uv: UV([u, v]),
                    tangent,
                },
            ]));
        }
    }

    debug_assert_eq!(data.len(), vertex_total_size);

    for z in 1..depth {
        for x in 1..width {
            data.extend_from_slice(bytemuck::cast_slice::<u32, _>(&[
                (x - 1) + (z - 1) * width,
                (x - 1) + (z - 0) * width,
                (x - 0) + (z - 0) * width,
                (x - 0) + (z - 0) * width,
                (x - 0) + (z - 1) * width,
                (x - 1) + (z - 1) * width,
            ]));
        }
    }

    debug_assert_eq!(data.len(), total_size);

    let buffer = ctx.create_buffer_static(
        BufferInfo {
            align: 255,
            size: total_size as u64,
            usage: buffer_usage,
        },
        &data,
    )?;

    let mesh = MeshBuilder::with_topology(PrimitiveTopology::TriangleList)
        .with_binding(buffer.clone(), 0, PositionNormalTangent3dUV::layout())
        .with_indices(buffer.clone(), vertex_total_size as u64, IndexType::U32)
        .build(index_count, vertex_count);

    Ok(mesh)
}

pub fn image_heightmap<P: Pixel>(
    image: &impl GenericImageView<Pixel = P>,
    factor: f32,
) -> (u32, u32, impl Fn(u32, u32) -> f32 + '_) {
    let (w, h) = image.dimensions();
    (w, h, move |x: u32, y: u32| {
        let pixel = image.get_pixel(x, y).to_luma()[0].to_f32().unwrap_or(0.0);
        let min = P::Subpixel::min_value().to_f32().unwrap_or(0.0);
        let max = P::Subpixel::max_value().to_f32().unwrap_or(1.0);

        std::f32::consts::E.powf(factor * (pixel - min) / (max - min))
    })
}

pub fn image_heightmap_alpha<P: Pixel>(
    image: &impl GenericImageView<Pixel = P>,
    factor: f32,
) -> (u32, u32, impl Fn(u32, u32) -> f32 + '_) {
    let (w, h) = image.dimensions();
    (w, h, move |x: u32, y: u32| {
        let pixel = image.get_pixel(x, y).to_luma_alpha()[1]
            .to_f32()
            .unwrap_or(0.0);
        let min = P::Subpixel::min_value().to_f32().unwrap_or(0.0);
        let max = P::Subpixel::max_value().to_f32().unwrap_or(1.0);

        factor * (pixel - min) / (max - min)
    })
}

pub struct TerrainRepr {
    heightmap: DynamicImage,
    material: MaterialRepr,
    buffer_usage: BufferUsage,
    factor: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum TerrainError {
    #[error(transparent)]
    ImageError(#[from] ImageError),

    #[error("Failed to deserialize `TerrainInfo`: `{source}`")]
    TerrainInfo {
        #[from]
        source: ron::Error,
    },

    #[error("Out of device memory")]
    OutOfMemory,

    #[error("Failed to load texture: `{source}`")]
    TextureError {
        #[from]
        source: goods::Error,
    },
}

impl From<OutOfMemory> for TerrainError {
    fn from(_: OutOfMemory) -> Self {
        TerrainError::OutOfMemory
    }
}

impl Asset for TerrainAsset {
    type Context = Context;
    type Error = TerrainError;
    type Repr = TerrainRepr;

    type BuildFuture = BoxFuture<'static, Result<Self, TerrainError>>;

    fn build(
        repr: TerrainRepr,
        ctx: &mut Context,
    ) -> BoxFuture<'static, Result<Self, TerrainError>> {
        let (w, h, f) = image_heightmap(&repr.heightmap, repr.factor);
        let shape = Arc::new(create_terrain_shape(w, h, &f));

        let mesh = create_terrain_mesh(w, h, &f, repr.buffer_usage, ctx);
        let material = repr.material.prebuild(ctx);

        Box::pin(async move {
            Ok(TerrainAsset {
                mesh: mesh?,
                shape,
                material: material?.finish().await?,
            })
        })
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct TerrainInfo {
    heightmap: String,

    #[serde(flatten)]
    material: MaterialInfo,

    factor: f32,
}

#[derive(Debug)]
pub struct TerrainFormat {
    pub raster: bool,
    pub blas: bool,
}

impl Format<TerrainAsset, AssetKey> for TerrainFormat {
    type DecodeFuture = BoxFuture<'static, Result<TerrainRepr, TerrainError>>;
    type Error = TerrainError;

    fn decode(
        self,
        key: AssetKey,
        bytes: Vec<u8>,
        assets: &Assets,
    ) -> BoxFuture<'static, Result<TerrainRepr, TerrainError>> {
        let info = match ron::de::from_bytes::<TerrainInfo>(&bytes) {
            Ok(info) => info,
            Err(err) => return Box::pin(ready(Err(err.into()))),
        };

        let heightmap_bytes =
            assets.load::<Box<[u8]>>(append_key(&key, &info.heightmap));
        let material = info.material.load(Some(&key), assets);

        let mut buffer_usage = BufferUsage::empty();

        if self.raster {
            buffer_usage |= BufferUsage::VERTEX | BufferUsage::INDEX;
        }

        if self.blas {
            buffer_usage |= BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT
                | BufferUsage::STORAGE
                | BufferUsage::DEVICE_ADDRESS;
        }

        let factor = info.factor;

        Box::pin(async move {
            let heightmap = load_from_memory(&heightmap_bytes.await?)?;

            Ok(TerrainRepr {
                heightmap,
                material,
                buffer_usage,
                factor,
            })
        })
    }
}

#[derive(Clone)]
pub struct TerrainAsset {
    pub mesh: Mesh,
    pub material: Material,
    pub shape: Arc<HeightField>,
}

/// Terrain entity consists of terrain marker and optionally
/// mesh, material and collider components.
///
/// Both mesh and collider can be created from same height image.
#[derive(Clone, Copy, Debug)]
pub struct Terrain;

impl Prefab for TerrainAsset {
    type Info = Global3;

    fn spawn(
        self,
        global: Global3,
        world: &mut World,
        resources: &mut TypeMap,
        entity: Entity,
    ) {
        let sets = resources
            .entry::<PhysicsData>()
            .or_insert_with(PhysicsData::new);

        let body = sets.bodies.insert(RigidBodyBuilder::new_static().build());
        let collider = sets.colliders.insert(
            ColliderBuilder::new(SharedShape(self.shape)).build(),
            body,
            &mut sets.bodies,
        );

        let _ = world.insert(
            entity,
            (
                Renderable {
                    mesh: self.mesh,
                    material: self.material,
                    // transform: None,
                },
                body,
                collider,
                global,
                Terrain,
            ),
        );
    }
}

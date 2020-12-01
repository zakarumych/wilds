use {
    super::{
        append_key,
        material::{MaterialInfo, MaterialRepr},
        ready, Asset, AssetKey, Assets, Format, Prefab,
    },
    crate::{
        physics::{BodyStatus, Colliders, RigidBodyDesc},
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
        BufferInfo, BufferUsage, IndexType, MemoryUsageFlags, OutOfMemory,
        PrimitiveTopology,
    },
    image::{
        load_from_memory, DynamicImage, GenericImageView, ImageError, Pixel,
    },
    nalgebra as na,
    ncollide3d::shape::{HeightField, ShapeHandle},
    nphysics3d::object::ColliderDesc,
    num_traits::{bounds::Bounded, cast::ToPrimitive},
    std::{convert::TryFrom as _, sync::Arc},
};

pub fn create_terrain_shape(
    width: u32,
    depth: u32,
    height: impl Fn(u32, u32) -> f32,
) -> HeightField<f32> {
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
    let mut data: Vec<u8> = Vec::new();

    let xoff = width as f32 * 0.5;
    let zoff = depth as f32 * 0.5;

    for z in 0..depth {
        for x in 0..width {
            let h = height(x, z);
            let h_n = if z == depth - 1 { h } else { height(x, z + 1) };
            let h_s = if z == 0 { h } else { height(x, z - 1) };
            let h_w = if x == 0 { h } else { height(x - 1, z) };
            let h_e = if x == width - 1 { h } else { height(x + 1, z) };

            let h_ne = match (width - x, depth - z) {
                (1, 1) => h,
                (1, _) => h_n,
                (_, 1) => h_e,
                _ => height(x + 1, z + 1),
            };
            let h_se = match (width - x, z) {
                (1, 0) => h,
                (1, _) => h_s,
                (_, 0) => h_e,
                _ => height(x + 1, z - 1),
            };
            let h_nw = match (x, depth - z) {
                (0, 1) => h,
                (0, _) => h_n,
                (_, 1) => h_w,
                _ => height(x - 1, z + 1),
            };
            let h_sw = match (x, z) {
                (0, 0) => h,
                (0, _) => h_s,
                (_, 0) => h_w,
                _ => height(x - 1, z - 1),
            };

            let shift_n = na::Vector3::from([0.0, h_n - h, 1.0]);
            let shift_s = na::Vector3::from([0.0, h_s - h, -1.0]);
            let shift_w = na::Vector3::from([-1.0, h_w - h, 0.0]);
            let shift_e = na::Vector3::from([1.0, h_e - h, 0.0]);

            let shift_ne = na::Vector3::from([1.0, h_ne - h, 1.0]);
            let shift_se = na::Vector3::from([1.0, h_se - h, -1.0]);
            let shift_nw = na::Vector3::from([-1.0, h_nw - h, 1.0]);
            let shift_sw = na::Vector3::from([-1.0, h_sw - h, -1.0]);

            let normal_ne = (shift_n.cross(&shift_e)
                + (shift_ne - shift_e).cross(&(shift_ne - shift_n)))
            .normalize();

            let normal_nw = (shift_w.cross(&shift_n)
                + (shift_nw - shift_n).cross(&(shift_nw - shift_w)))
            .normalize();

            let normal_se = (shift_e.cross(&shift_s)
                + (shift_se - shift_s).cross(&(shift_se - shift_e)))
            .normalize();

            let normal_sw = (shift_s.cross(&shift_w)
                + (shift_sw - shift_w).cross(&(shift_sw - shift_s)))
            .normalize();

            let tangent = Tangent3d([1.0, 0.0, 0.0, 1.0]);

            let xf = x as f32 - xoff;
            let zf = z as f32 - zoff;

            let u = x as f32;
            let v = z as f32;

            data.extend_from_slice(bytemuck::cast_slice(&[
                PositionNormalTangent3dUV {
                    position: Position3d([
                        xf - 0.5,
                        (h + h_s + h_w + h_sw) / 4.0,
                        zf - 0.5,
                    ]),
                    normal: Normal3d(normal_sw.into()),
                    uv: UV([u, v]),
                    tangent,
                },
                PositionNormalTangent3dUV {
                    position: Position3d([
                        xf + 0.5,
                        (h + h_s + h_e + h_se) / 4.0,
                        zf - 0.5,
                    ]),
                    normal: Normal3d(normal_se.into()),
                    uv: UV([u + 1.0, v]),
                    tangent,
                },
                PositionNormalTangent3dUV {
                    position: Position3d([
                        xf - 0.5,
                        (h + h_n + h_w + h_nw) / 4.0,
                        zf + 0.5,
                    ]),
                    normal: Normal3d(normal_nw.into()),
                    uv: UV([u, v + 1.0]),
                    tangent,
                },
                PositionNormalTangent3dUV {
                    position: Position3d([
                        xf + 0.5,
                        (h + h_n + h_e + h_ne) / 4.0,
                        zf + 0.5,
                    ]),
                    normal: Normal3d(normal_ne.into()),
                    uv: UV([u + 1.0, v + 1.0]),
                    tangent,
                },
            ]));
        }
    }

    let indices_offset = u64::try_from(data.len()).map_err(|_| OutOfMemory)?;

    let mut index: u32 = 0;
    for _ in 0..depth {
        for _ in 0..width {
            data.extend_from_slice(bytemuck::cast_slice(&[
                index + 0,
                index + 2,
                index + 3,
                index + 3,
                index + 1,
                index + 0,
            ]));
            index += 4;
        }
    }

    let data_size = u64::try_from(data.len()).map_err(|_| OutOfMemory)?;

    let buffer = ctx.create_buffer_static(
        BufferInfo {
            align: 255,
            size: data_size,
            usage: buffer_usage,
            memory: MemoryUsageFlags::empty(),
        },
        &data,
    )?;

    let squares = width * depth;

    let vertex_count = squares * 4;
    let index_count = squares * 6;

    let mesh = MeshBuilder::with_topology(PrimitiveTopology::TriangleList)
        .with_binding(buffer.clone(), 0, PositionNormalTangent3dUV::layout())
        .with_indices(buffer.clone(), indices_offset, IndexType::U32)
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
            buffer_usage |= BufferUsage::RAY_TRACING
                | BufferUsage::STORAGE
                | BufferUsage::SHADER_DEVICE_ADDRESS;
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
    pub shape: Arc<HeightField<f32>>,
}

/// Terrain entity consists of terrain marker and optionally
/// mesh, material and collider components.
///
/// Both mesh and collider can be created from same height image.
#[derive(Clone, Copy, Debug)]
pub struct Terrain;

impl Prefab for TerrainAsset {
    type Info = Global3;

    fn spawn(self, global: Global3, world: &mut World, entity: Entity) {
        let rigid_body = RigidBodyDesc::<f32>::new()
            .status(BodyStatus::Static)
            .build();

        let _ = world.insert(
            entity,
            (
                Renderable {
                    mesh: self.mesh,
                    material: self.material,
                    // transform: None,
                },
                rigid_body,
                Colliders::from(
                    ColliderDesc::new(ShapeHandle::from_arc(self.shape))
                        .margin(0.01),
                ),
                global,
                Terrain,
            ),
        );
    }
}

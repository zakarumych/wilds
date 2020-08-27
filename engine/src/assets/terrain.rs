use {
    super::{AssetKey, Assets, Prefab},
    crate::{
        physics::{BodyStatus, Colliders, RigidBodyDesc},
        renderer::{
            Context, Material, Mesh, MeshBuilder, Normal3d, Position3d,
            PositionNormalTangent3dUV, Renderable, Tangent3d, VertexType as _,
            UV,
        },
        scene::Global3,
    },
    goods::{ready, Format, Ready, SyncAsset},
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
    heightmap: impl Fn(u32, u32) -> f32,
) -> HeightField<f32> {
    let mut matrix: na::DMatrix<f32> = na::DMatrix::zeros_generic(
        na::Dynamic::new(depth as usize),
        na::Dynamic::new(width as usize),
    );

    for x in 0..width {
        for y in 0..depth {
            matrix[(y as usize, x as usize)] = heightmap(x, y);
        }
    }

    HeightField::new(matrix, na::Vector3::new(width as f32, 1.0, depth as f32))
}

pub fn create_terrain_mesh(
    width: u32,
    depth: u32,
    heightmap: impl Fn(u32, u32) -> f32,
    buffer_usage: BufferUsage,
    ctx: &mut Context,
) -> Result<Mesh, OutOfMemory> {
    let mut data: Vec<u8> = Vec::new();
    let mut indices_offset = 0;

    let xoff = (width - 1) as f32 * 0.5;
    let zoff = (depth - 1) as f32 * 0.5;

    if width > 1 && depth > 1 {
        for z in 0..depth - 1 {
            for x in 0..width - 1 {
                let xf = x as f32 - xoff;
                let zf = z as f32 - zoff;

                let pos = na::Vector3::from([xf, heightmap(x, z), zf]);
                let posx =
                    na::Vector3::from([xf + 1.0, heightmap(x + 1, z), zf]);
                let posy =
                    na::Vector3::from([xf, heightmap(x, z + 1), zf + 1.0]);
                let posxy = na::Vector3::from([
                    xf + 1.0,
                    heightmap(x + 1, z + 1),
                    zf + 1.0,
                ]);

                let n1 = (pos - posx).cross(&(posy - pos));
                let n2 = (posy - posxy).cross(&(posxy - posx));
                let n3 = (pos - posx).cross(&(posxy - posx));
                let n4 = (posy - posxy).cross(&(posy - pos));

                let normal = (n1 + n2 + n3 + n4).normalize();
                let tangent = Tangent3d([1.0, 0.0, 0.0, 1.0]);

                data.extend_from_slice(bytemuck::cast_slice(&[
                    PositionNormalTangent3dUV {
                        position: Position3d(pos.into()),
                        normal: Normal3d(normal.into()),
                        uv: UV([xf, zf]),
                        tangent,
                    },
                    PositionNormalTangent3dUV {
                        position: Position3d(posx.into()),
                        normal: Normal3d(normal.into()),
                        uv: UV([xf + 1.0, zf]),
                        tangent,
                    },
                    PositionNormalTangent3dUV {
                        position: Position3d(posy.into()),
                        normal: Normal3d(normal.into()),
                        uv: UV([xf, zf + 1.0]),
                        tangent,
                    },
                    PositionNormalTangent3dUV {
                        position: Position3d(posxy.into()),
                        normal: Normal3d(normal.into()),
                        uv: UV([xf + 1.0, zf + 1.0]),
                        tangent,
                    },
                ]));
            }
        }

        indices_offset = data.len();

        let mut index: u32 = 0;
        for _ in 0..depth - 1 {
            for _ in 0..width - 1 {
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
    } else {
        tracing::warn!("Generating empty terrain mesh");
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

    let squares = if width > 1 && depth > 1 {
        (depth - 1) * (width - 1)
    } else {
        0
    };

    let vertex_count = squares * 4;
    let index_count = squares * 6;

    let mesh = MeshBuilder::with_topology(PrimitiveTopology::TriangleList)
        .with_binding(buffer.clone(), 0, PositionNormalTangent3dUV::layout())
        .with_indices(buffer.clone(), indices_offset as u64, IndexType::U32)
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

        factor * (pixel - min) / (max - min)
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
    image: DynamicImage,
    buffer_usage: BufferUsage,
    factor: f32,
}

impl SyncAsset for TerrainAsset {
    type Context = Context;
    type Error = OutOfMemory;
    type Repr = TerrainRepr;

    fn build(
        repr: TerrainRepr,
        ctx: &mut Context,
    ) -> Result<Self, OutOfMemory> {
        let (w, h, f) = image_heightmap(&repr.image, repr.factor);
        let mesh = create_terrain_mesh(w, h, &f, repr.buffer_usage, ctx)?;
        let shape = Arc::new(create_terrain_shape(w, h, &f));

        Ok(TerrainAsset { mesh, shape })
    }
}

#[derive(Debug)]
pub struct TerrainFormat {
    pub raster: bool,
    pub blas: bool,
    pub factor: f32,
}

impl Format<TerrainAsset, AssetKey> for TerrainFormat {
    type DecodeFuture = Ready<Result<TerrainRepr, ImageError>>;
    type Error = ImageError;

    fn decode(
        self,
        _: AssetKey,
        bytes: Vec<u8>,
        _: &Assets,
    ) -> Self::DecodeFuture {
        let mut buffer_usage = BufferUsage::empty();
        if self.raster {
            buffer_usage |= BufferUsage::VERTEX | BufferUsage::INDEX;
        }
        if self.blas {
            buffer_usage |= BufferUsage::RAY_TRACING
                | BufferUsage::STORAGE
                | BufferUsage::SHADER_DEVICE_ADDRESS;
        }

        ready(load_from_memory(&bytes).map(|image| TerrainRepr {
            image,
            buffer_usage,
            factor: self.factor,
        }))
    }
}

#[derive(Clone)]
pub struct TerrainAsset {
    pub mesh: Mesh,
    pub shape: Arc<HeightField<f32>>,
}

/// Terrain entity consists of terrain marker and optionally
/// mesh, material and collider components.
///
/// Both mesh and collider can be created from same heightmap image.
#[derive(Clone, Copy, Debug)]
pub struct Terrain;

impl Prefab for TerrainAsset {
    type Info = na::Isometry3<f32>;

    fn spawn(self, iso: na::Isometry3<f32>, world: &mut World, entity: Entity) {
        let rigid_body = RigidBodyDesc::<f32>::new()
            .status(BodyStatus::Static)
            .build();

        let _ = world.insert(
            entity,
            (
                Renderable {
                    mesh: self.mesh,
                    material: Material::color([0.3, 0.5, 0.7, 1.0]),
                    // transform: None,
                },
                rigid_body,
                Colliders::from(
                    ColliderDesc::new(ShapeHandle::from_arc(self.shape))
                        .margin(0.01),
                ),
                Global3::from_iso(iso),
                Terrain,
            ),
        );
    }
}

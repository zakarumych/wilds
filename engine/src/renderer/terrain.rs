use {
    super::{
        mesh::MeshData,
        vertex::{
            Normal3d, Position3d, PositionNormalTangent3dUV, Tangent3d, UV,
        },
    },
    illume::{
        Buffer, BufferInfo, BufferUsage, MemoryUsageFlags, OutOfMemory,
        PrimitiveTopology,
    },
    image::GenericImageView,
    std::{convert::TryInto as _, mem::size_of_val},
    ultraviolet::Vec3,
};

/// Terrain mesh buffers.
#[derive(Debug)]
pub struct TerrainMesh {
    pub index_buffer: Buffer,
    pub vertex_buffer: Buffer,
}

pub fn generate_terrain_mesh(
    hightmap: &impl GenericImageView<Pixel = image::Rgba<u8>>,
) -> MeshData {
    let (width, height) = hightmap.dimensions();

    let mut indices = Vec::new();

    // let index_of = |x, y| y * width + x;

    // for y in 0..height - 1 {
    //     if y % 2 > 0 {
    //         for x in (0..width).rev() {
    //             indices.push(index_of(x, y));

    //             indices.push(index_of(x, y + 1));
    //         }

    //         if y < height - 2 {
    //             indices.push(index_of(width - 1, y + 1));
    //         }
    //     } else {
    //         for x in 0..width {
    //             indices.push(index_of(x, y));

    //             indices.push(index_of(x, y + 1));
    //         }

    //         if y < height - 2 {
    //             indices.push(index_of(0, y + 1));
    //         }
    //     }
    // }


    let mut vertices = Vec::new();

    for y in 0 .. height - 1 {
        for x in 0 .. width - 1 {
            let pos = [x as f32, y as f32, hightmap.get_pixel(x, y)[3] as f32];
            let posx = [x as f32, y as f32, hightmap.get_pixel(x+1, y)[3] as f32];
            let posy = [x as f32, y as f32, hightmap.get_pixel(x, y+1)[3] as f32];
            let posxy = [x as f32, y as f32, hightmap.get_pixel(x+1, y+1)[3] as f32];

            let o = Vec3::from(pos);
            let x = Vec3::from(posx);
            let y = Vec3::from(posy);
            let xy = Vec3::from(posxy);

            let n1 = (x - o).cross(y - o);
            let n2 = (y - xy).cross(x - xy);
            let n3 = (x - o).cross(xy - x);
            let n4 = (y - xy).cross(o - y);

            let normal = (n1 + n2 + n3 + n4).normalized();
            let uv = 

            PositionNormalTangent3dUV {

            }
        }
    }

    let vertices = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let rgba: image::Rgba<u8> = hightmap.get_pixel(x, y);

                let norm = 

            })
        })
        .collect::<Vec<_>>();

    MeshData { bindings: vec![] }
}

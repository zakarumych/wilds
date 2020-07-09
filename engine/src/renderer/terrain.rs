use illume::{
    Buffer, BufferInfo, BufferUsage, Device, MemoryUsageFlags, OutOfMemory,
    PrimitiveTopology,
};
use std::{convert::TryInto as _, mem::size_of_val};

#[cfg(feature = "image")]
use image::GenericImageView;

/// Terrain mesh buffers.
#[derive(Debug)]
pub struct TerrainMesh {
    pub index_buffer: Buffer,
    pub vertex_buffer: Buffer,
}

#[cfg(feature = "image")]
pub fn generate_terrain_mesh(
    device: &Device,
    hightmap: &impl GenericImageView<Pixel = image::Rgba<u8>>,
) -> Result<TerrainMesh, OutOfMemory> {
    let (width, height) = hightmap.dimensions();

    let mut indices = Vec::new();

    let index_of = |x, y| y * width + x;

    for y in 0..height - 1 {
        if y % 2 > 0 {
            for x in (0..width).rev() {
                indices.push(index_of(x, y));

                indices.push(index_of(x, y + 1));
            }

            if y < height - 2 {
                indices.push(index_of(width - 1, y + 1));
            }
        } else {
            for x in 0..width {
                indices.push(index_of(x, y));

                indices.push(index_of(x, y + 1));
            }

            if y < height - 2 {
                indices.push(index_of(0, y + 1));
            }
        }
    }

    let vertices = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let rgba: image::Rgba<u8> = hightmap.get_pixel(x, y);

                [x as u16, y as u16, rgba[3] as u16]
            })
        })
        .collect::<Vec<_>>();

    let index_buffer = device.create_buffer_static(
        BufferInfo {
            align: 1,
            size: size_of_val(&indices[..])
                .try_into()
                .map_err(|_| OutOfMemory)?,
            usage: BufferUsage::INDEX,
            memory: MemoryUsageFlags::Device,
        },
        &indices[..],
    )?;

    let vertex_buffer = device.create_buffer_static(
        BufferInfo {
            align: 1,
            size: size_of_val(&vertices[..])
                .try_into()
                .map_err(|_| OutOfMemory)?,
            usage: BufferUsage::VERTEX,
            memory: MemoryUsageFlags::Device,
        },
        &vertices[..],
    )?;

    Ok(TerrainMesh {
        index_buffer,
        vertex_buffer,
    })
}

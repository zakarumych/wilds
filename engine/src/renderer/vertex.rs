use bytemuck::{Pod, Zeroable};
use byteorder::ByteOrder;
use illume::{
    Format, VertexInputAttribute, VertexInputBinding, VertexInputRate,
};
use std::{
    borrow::Cow,
    marker::PhantomData,
    mem::{size_of, size_of_val},
};

/// Describes single vertex location.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct VertexLocation {
    /// Specifies how data is interpreted for attibutes.
    /// Attribute component types in vertex shader must match base type of the
    /// format.
    pub format: Format,

    /// Offset of data in vertex buffer element.
    pub offset: u32,
}

/// Describes layout of vertex buffer element.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct VertexLayout {
    pub locations: Cow<'static, [VertexLocation]>,
    pub stride: u32,
    pub rate: VertexInputRate,
}

pub trait FromBytes {
    /// Loads value from raw bytes slice.
    /// This function may expect that bytes len equals size of the type.
    ///
    /// # Panics
    ///
    /// This function is expected to panic if bytes len is invalid.
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self
    where
        Self: Sized;

    /// Loads multiple values from raw bytes slice.
    /// For each value bytes offset is advanced by `stride`.
    fn from_bytes_iter<E: ByteOrder>(
        bytes: &[u8],
        stride: usize,
    ) -> FromBytesIter<Self, E>
    where
        Self: Sized,
    {
        FromBytesIter {
            bytes,
            stride,
            marker: PhantomData,
        }
    }
}

impl FromBytes for u16 {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        E::read_u16(bytes)
    }
}

impl FromBytes for u32 {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        E::read_u32(bytes)
    }
}

/// Trait for vertex layouts.
pub trait VertexType: FromBytes + Pod {
    const NAME: &'static str;
    const LOCATIONS: &'static [VertexLocation];
    const RATE: VertexInputRate;

    /// Get layout of the vertex type.
    /// FIXME: make function const when stable.
    fn layout() -> VertexLayout
    where
        Self: Sized,
    {
        VertexLayout {
            locations: Cow::Borrowed(Self::LOCATIONS),
            stride: size_of::<Self>() as u32,
            rate: Self::RATE,
        }
    }

    /// Get layout of the vertex value.
    /// FIXME: make function const when stable.
    fn layout_of_val(&self) -> VertexLayout {
        VertexLayout {
            locations: Cow::Borrowed(Self::LOCATIONS),
            stride: size_of_val(self) as u32,
            rate: Self::RATE,
        }
    }
}

/// Attribute for vertex position in 3d world.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Position3d(pub [f32; 3]);

unsafe impl Zeroable for Position3d {}
unsafe impl Pod for Position3d {}

impl FromBytes for Position3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut xyz = [0.0; 3];
        E::read_f32_into(bytes, &mut xyz);
        Position3d(xyz)
    }
}

impl VertexType for Position3d {
    const LOCATIONS: &'static [VertexLocation] = &[VertexLocation {
        format: Format::RGB32Sfloat,
        offset: 0,
    }];
    const NAME: &'static str = "Position3d";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

/// Attribute for vertex normal in 3d world.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Normal3d(pub [f32; 3]);

unsafe impl Zeroable for Normal3d {}
unsafe impl Pod for Normal3d {}

impl FromBytes for Normal3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut xyz = [0.0; 3];
        E::read_f32_into(bytes, &mut xyz);
        Normal3d(xyz)
    }
}

impl VertexType for Normal3d {
    const LOCATIONS: &'static [VertexLocation] = &[VertexLocation {
        format: Format::RGB32Sfloat,
        offset: 0,
    }];
    const NAME: &'static str = "Normal3d";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

/// Attribute for vertex position in 3d world.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Tangent3d(pub [f32; 4]);

unsafe impl Zeroable for Tangent3d {}
unsafe impl Pod for Tangent3d {}

impl FromBytes for Tangent3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut xyz = [0.0; 4];
        E::read_f32_into(bytes, &mut xyz);
        Tangent3d(xyz)
    }
}

impl VertexType for Tangent3d {
    const LOCATIONS: &'static [VertexLocation] = &[VertexLocation {
        format: Format::RGBA32Sfloat,
        offset: 0,
    }];
    const NAME: &'static str = "Tangent3d";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

/// Attribute for vertex color.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct Color(pub [f32; 4]);

unsafe impl Zeroable for Color {}
unsafe impl Pod for Color {}

impl FromBytes for Color {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut rgba = [0.0; 4];
        E::read_f32_into(bytes, &mut rgba);
        Color(rgba)
    }
}

impl VertexType for Color {
    const LOCATIONS: &'static [VertexLocation] = &[VertexLocation {
        format: Format::RGBA32Sfloat,
        offset: 0,
    }];
    const NAME: &'static str = "Color";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

/// Attribute for texture coordinates.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct UV(pub [f32; 2]);

unsafe impl Zeroable for UV {}
unsafe impl Pod for UV {}

impl FromBytes for UV {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut uv = [0.0; 2];
        E::read_f32_into(bytes, &mut uv);
        UV(uv)
    }
}

impl VertexType for UV {
    const LOCATIONS: &'static [VertexLocation] = &[VertexLocation {
        format: Format::RG32Sfloat,
        offset: 0,
    }];
    const NAME: &'static str = "UV";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Position3dUV {
    pub position: Position3d,
    pub uv: UV,
}

unsafe impl Zeroable for Position3dUV {}
unsafe impl Pod for Position3dUV {}

impl FromBytes for Position3dUV {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 5];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, u, v] = array;
        let uv = UV([u, v]);

        Position3dUV { position, uv }
    }
}

impl VertexType for Position3dUV {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RG32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
    ];
    const NAME: &'static str = "Position3dUV";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Position3dColor {
    pub position: Position3d,
    pub color: Color,
}

unsafe impl Zeroable for Position3dColor {}
unsafe impl Pod for Position3dColor {}

impl FromBytes for Position3dColor {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 7];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, r, g, b, a] = array;
        let color = Color([r, g, b, a]);

        Position3dColor { position, color }
    }
}

impl VertexType for Position3dColor {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
    ];
    const NAME: &'static str = "Position3dColor";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PositionNormal3d {
    pub position: Position3d,
    pub normal: Normal3d,
}

unsafe impl Zeroable for PositionNormal3d {}
unsafe impl Pod for PositionNormal3d {}

impl FromBytes for PositionNormal3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 6];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, x, y, z] = array;
        let normal = Normal3d([x, y, z]);

        PositionNormal3d { position, normal }
    }
}

impl VertexType for PositionNormal3d {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
    ];
    const NAME: &'static str = "PositionNormal3d";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PositionNormalTangent3d {
    pub position: Position3d,
    pub normal: Normal3d,
    pub tangent: Tangent3d,
}

unsafe impl Zeroable for PositionNormalTangent3d {}
unsafe impl Pod for PositionNormalTangent3d {}

impl FromBytes for PositionNormalTangent3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 10];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, x, y, z, ..] = array;
        let normal = Normal3d([x, y, z]);

        let [_, _, _, _, _, _, x, y, z, w] = array;
        let tangent = Tangent3d([x, y, z, w]);

        PositionNormalTangent3d {
            position,
            normal,
            tangent,
        }
    }
}

impl VertexType for PositionNormalTangent3d {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<Position3d>() as u32
                + size_of::<Normal3d>() as u32,
        },
    ];
    const NAME: &'static str = "PositionNormalTangent3d";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PositionNormal3dUV {
    pub position: Position3d,
    pub normal: Normal3d,
    pub uv: UV,
}

unsafe impl Zeroable for PositionNormal3dUV {}
unsafe impl Pod for PositionNormal3dUV {}

impl FromBytes for PositionNormal3dUV {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 8];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, x, y, z, ..] = array;
        let normal = Normal3d([x, y, z]);

        let [_, _, _, _, _, _, u, v] = array;
        let uv = UV([u, v]);

        PositionNormal3dUV {
            position,
            normal,
            uv,
        }
    }
}

impl VertexType for PositionNormal3dUV {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
        VertexLocation {
            format: Format::RG32Sfloat,
            offset: size_of::<Position3d>() as u32
                + size_of::<Normal3d>() as u32,
        },
    ];
    const NAME: &'static str = "PositionNormal3dUV";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PositionNormalTangent3dUV {
    pub position: Position3d,
    pub normal: Normal3d,
    pub tangent: Tangent3d,
    pub uv: UV,
}

unsafe impl Zeroable for PositionNormalTangent3dUV {}
unsafe impl Pod for PositionNormalTangent3dUV {}

impl FromBytes for PositionNormalTangent3dUV {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 12];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, x, y, z, ..] = array;
        let normal = Normal3d([x, y, z]);

        let [_, _, _, _, _, _, x, y, z, w, ..] = array;
        let tangent = Tangent3d([x, y, z, w]);

        let [_, _, _, _, _, _, _, _, _, _, u, v] = array;
        let uv = UV([u, v]);

        PositionNormalTangent3dUV {
            position,
            normal,
            tangent,
            uv,
        }
    }
}

impl VertexType for PositionNormalTangent3dUV {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<Position3d>() as u32
                + size_of::<Normal3d>() as u32,
        },
        VertexLocation {
            format: Format::RG32Sfloat,
            offset: size_of::<Position3d>() as u32
                + size_of::<Normal3d>() as u32
                + size_of::<Tangent3d>() as u32,
        },
    ];
    const NAME: &'static str = "PositionNormalTangent3dUV";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PositionNormal3dColor {
    pub position: Position3d,
    pub normal: Normal3d,
    pub color: Color,
}

unsafe impl Zeroable for PositionNormal3dColor {}
unsafe impl Pod for PositionNormal3dColor {}

impl FromBytes for PositionNormal3dColor {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut array = [0.0; 10];
        E::read_f32_into(bytes, &mut array);

        let [x, y, z, ..] = array;
        let position = Position3d([x, y, z]);

        let [_, _, _, x, y, z, ..] = array;
        let normal = Normal3d([x, y, z]);

        let [_, _, _, _, _, _, r, g, b, a] = array;
        let color = Color([r, g, b, a]);

        PositionNormal3dColor {
            position,
            normal,
            color,
        }
    }
}

impl VertexType for PositionNormal3dColor {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: 0,
        },
        VertexLocation {
            format: Format::RGB32Sfloat,
            offset: size_of::<Position3d>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<Position3d>() as u32
                + size_of::<Normal3d>() as u32,
        },
    ];
    const NAME: &'static str = "PositionNormal3dColor";
    const RATE: VertexInputRate = VertexInputRate::Vertex;
}

/// Attribute for instance 3d transformation.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct Transformation3d([[f32; 4]; 4]);

unsafe impl Zeroable for Transformation3d {}
unsafe impl Pod for Transformation3d {}

impl FromBytes for Transformation3d {
    fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Self {
        let mut mat = [0.0; 16];
        E::read_f32_into(bytes, &mut mat);
        Transformation3d([
            [mat[0], mat[1], mat[2], mat[3]],
            [mat[4], mat[5], mat[6], mat[7]],
            [mat[8], mat[9], mat[10], mat[11]],
            [mat[12], mat[13], mat[14], mat[15]],
        ])
    }
}

impl VertexType for Transformation3d {
    const LOCATIONS: &'static [VertexLocation] = &[
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<[[f32; 4]; 0]>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<[[f32; 4]; 1]>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<[[f32; 4]; 2]>() as u32,
        },
        VertexLocation {
            format: Format::RGBA32Sfloat,
            offset: size_of::<[[f32; 4]; 3]>() as u32,
        },
    ];
    const NAME: &'static str = "Transformation3d";
    const RATE: VertexInputRate = VertexInputRate::Instance;
}

pub fn vertex_layouts_for_pipeline(
    layouts: &[VertexLayout],
) -> (Vec<VertexInputBinding>, Vec<VertexInputAttribute>) {
    let mut next_location = 0;

    let mut attributes = Vec::new();

    let bindings = layouts
        .iter()
        .enumerate()
        .map(|(binding, layout)| {
            attributes.extend(layout.locations.iter().map(|layout| {
                next_location += 1;

                VertexInputAttribute {
                    location: next_location - 1,
                    format: layout.format,
                    offset: layout.offset,
                    binding: binding as u32,
                }
            }));

            VertexInputBinding {
                stride: layout.stride,
                rate: layout.rate,
            }
        })
        .collect();

    (bindings, attributes)
}

#[cfg(feature = "genmesh")]
mod gm {
    use super::*;
    use genmesh::Vertex;

    impl From<Vertex> for Position3d {
        fn from(v: Vertex) -> Self {
            Position3d([v.pos.x, v.pos.y, v.pos.z])
        }
    }

    impl From<Vertex> for Normal3d {
        fn from(v: Vertex) -> Self {
            Normal3d([v.normal.x, v.normal.y, v.normal.z])
        }
    }

    impl From<Vertex> for PositionNormal3d {
        fn from(v: Vertex) -> Self {
            PositionNormal3d {
                position: v.into(),
                normal: v.into(),
            }
        }
    }
}

/// Iterator that reads vertices from bytes slice.
#[derive(Clone, Debug)]
pub struct FromBytesIter<'a, T, E> {
    bytes: &'a [u8],
    stride: usize,
    marker: PhantomData<fn(Option<E>) -> T>,
}

impl<T, E> Iterator for FromBytesIter<'_, T, E>
where
    T: FromBytes,
    E: ByteOrder,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.bytes.len() >= size_of::<T>() {
            let v = T::from_bytes::<E>(&self.bytes[..size_of::<T>()]);
            if self.bytes.len() >= self.stride {
                self.bytes = &self.bytes[self.stride..];
            } else {
                self.bytes = &[];
            }
            Some(v)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.len()
    }

    fn last(self) -> Option<T>
    where
        Self: Sized,
    {
        if self.bytes.len() >= size_of::<T>() {
            let offset = self.bytes.len() - size_of::<T>();
            Some(T::from_bytes::<E>(
                &self.bytes[offset - (offset % self.stride)..]
                    [..size_of::<T>()],
            ))
        } else {
            None
        }
    }

    fn nth(&mut self, n: usize) -> Option<T> {
        if self.bytes.len() >= n * self.stride + size_of::<T>() {
            self.bytes = &self.bytes[n * self.stride..];
            let v = T::from_bytes::<E>(&self.bytes[..size_of::<T>()]);
            if self.bytes.len() >= self.stride {
                self.bytes = &self.bytes[self.stride..];
            } else {
                self.bytes = &[];
            }
            Some(v)
        } else {
            self.bytes = &[];

            None
        }
    }
}

impl<T, E> ExactSizeIterator for FromBytesIter<'_, T, E>
where
    T: FromBytes,
    E: ByteOrder,
{
    fn len(&self) -> usize {
        if self.bytes.len() > size_of::<T>() {
            (self.bytes.len() - size_of::<T>()) / self.stride + 1
        } else {
            0
        }
    }
}

impl<T, E> std::iter::FusedIterator for FromBytesIter<'_, T, E>
where
    T: FromBytes,
    E: ByteOrder,
{
}

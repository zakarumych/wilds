use crate::resource::{Handle, ResourceTrait};
use ordered_float::OrderedFloat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum CompareOp {
    /// Never passes.
    Never,

    /// Passes if fragment's depth is less than stored.
    Less,

    /// Passes if fragment's depth is equal to stored.
    Equal,

    /// Passes if fragment's depth is less than or equal to stored.
    LessOrEqual,

    /// Passes if fragment's depth is greater than stored.
    Greater,

    /// Passes if fragment's depth is not equal to stored.
    NotEqual,

    /// Passes if fragment's depth is greater than or equal to stored.
    GreaterOrEqual,

    /// Always passes.
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum Filter {
    Nearest,
    Linear,
    // Cubic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum MipmapMode {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum SamplerAddressMode {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
    ClampToBorder,
    MirrorClampToEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum BorderColor {
    FloatTransparentBlack,
    IntTransparentBlack,
    FloatOpaqueBlack,
    IntOpaqueBlack,
    FloatOpaqueWhite,
    IntOpaqueWhite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct SamplerInfo {
    pub mag_filter: Filter,
    pub min_filter: Filter,
    pub mipmap_mode: MipmapMode,
    pub address_mode_u: SamplerAddressMode,
    pub address_mode_v: SamplerAddressMode,
    pub address_mode_w: SamplerAddressMode,
    pub mip_lod_bias: OrderedFloat<f32>,
    pub max_anisotropy: Option<OrderedFloat<f32>>,
    pub compare_op: Option<CompareOp>,
    pub min_lod: OrderedFloat<f32>,
    pub max_lod: OrderedFloat<f32>,
    pub border_color: BorderColor,
    pub unnormalized_coordinates: bool,
}

define_handle! {
    pub struct Sampler(SamplerInfo);
}

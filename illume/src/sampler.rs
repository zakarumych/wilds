use {erupt::vk1_0, ordered_float::OrderedFloat};

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

impl Default for SamplerInfo {
    fn default() -> Self {
        SamplerInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            mipmap_mode: MipmapMode::Nearest,
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            address_mode_w: SamplerAddressMode::Repeat,
            mip_lod_bias: 0.0.into(),
            max_anisotropy: None,
            compare_op: None,
            min_lod: 0.0.into(),
            max_lod: 1000.0.into(),
            border_color: BorderColor::FloatOpaqueBlack,
            unnormalized_coordinates: false,
        }
    }
}

define_handle! {
    pub struct Sampler {
        pub info: SamplerInfo,
        handle: vk1_0::Sampler,
    }
}

pub use crate::backend::Buffer;
use crate::{align_up, memory::MemoryUsageFlags};

bitflags::bitflags! {
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct BufferUsage: u32 {
        const TRANSFER_SRC = 0x00000001;
        const TRANSFER_DST = 0x00000002;
        const UNIFORM_TEXEL = 0x00000004;
        const STORAGE_TEXEL = 0x00000008;
        const UNIFORM = 0x00000010;
        const STORAGE = 0x00000020;
        const INDEX = 0x00000040;
        const VERTEX = 0x00000080;
        const INDIRECT = 0x00000100;
        const CONDITIONAL_RENDERING = 0x00000200;
        const RAY_TRACING = 0x00000400;
        const TRANSFORM_FEEDBACK = 0x00000800;
        const TRANSFORM_FEEDBACK_COUNTER = 0x00001000;
        const SHADER_DEVICE_ADDRESS = 0x00020000;
    }
}

/// Information required to create a buffer.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct BufferInfo {
    /// Alignment mask for content buffer can hold.
    pub align: u64,

    /// Size of content buffer can hold.
    pub size: u64,

    /// Usage types supported by buffer.
    pub usage: BufferUsage,

    /// Memory usage pattern.
    pub memory: MemoryUsageFlags,
}

impl BufferInfo {
    #[inline(always)]
    pub(crate) fn is_valid(&self) -> bool {
        let is_mask = self
            .align
            .checked_add(1)
            .map_or(false, u64::is_power_of_two);

        is_mask && (align_up(self.align, self.size).is_some())
    }
}

/// Buffer region with specified stride value.
/// Currently used in `Encoder::trace_rays`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StridedBufferRegion {
    pub buffer: Buffer,
    pub offset: u64,
    pub size: u64,
    pub stride: u64,
}

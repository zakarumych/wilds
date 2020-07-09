use {
    erupt::vk1_0::{MemoryPropertyFlags, MemoryType},
    tinyvec::ArrayVec,
};

bitflags::bitflags! {
    /// Memory usage type.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct UsageFlags: u8 {
        /// Hints allocator to find memory with faster device access.
        /// If no flags is specified than `FAST_DEVICE_ACCESS` is implicitly added.
        const FAST_DEVICE_ACCESS = 0x00000001;

        /// Memory will be accessed from host.
        const HOST_ACCESS = 0x00000002;

        /// Hints allocator that memory will be used for data uploading.
        /// Allocator will use faster allocation method assuming that
        /// memory will be deallocated soon after uploading completes.
        /// It is OK to use it for multiple subsequent uploadings.
        /// If `DOWNLOAD` flag is not set then allocator will assume
        /// host will access memory in write-only manner and may
        /// pick not host-cached.
        /// Implies `HOST_ACCESS`.
        const UPLOAD = 0x00000004;

        /// Hints allocator that memory will be used for data downloading.
        /// Allocator will use faster allocation method assuming that
        /// memory will be deallocated soon after downloading completes.
        /// It is OK to use it for multiple subsequent downloadings.
        /// Allocator will strongly prefer host-cached memory.
        /// Implies `HOST_ACCESS`.
        const DOWNLOAD = 0x00000008;

        /// Requests memory that can be addressed with `u64`.
        /// Allows fetching device address for resources bound to that memory.
        const DEVICE_ADDRESS = 0x00000010;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryForUsage {
    mask: u32,
    types: ArrayVec<[u32; 32]>,
}

impl MemoryForUsage {
    /// Returns mask with bits set for memory type indices that support the
    /// usage.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    /// Returns slice of memory type indices that support the usage.
    /// Earlier memory type has priority over later.
    pub fn types(&self) -> &[u32] {
        &self.types
    }

    pub fn for_usage(
        usage: UsageFlags,
        memory_types: &[MemoryType],
    ) -> MemoryForUsage {
        // Find memory types for `Device` usage.
        let mut types = memory_types
            .iter()
            .enumerate()
            .filter_map(|(index, memory_type)| {
                if Self::compatible(usage, memory_type.property_flags) {
                    Some(index as u32)
                } else {
                    None
                }
            })
            .collect::<ArrayVec<[u32; 32]>>();

        types.sort_by_key(|&index| {
            Self::priority(usage, memory_types[index as usize].property_flags)
        });

        let mask = types.iter().fold(0u32, |mask, index| mask | 1u32 << index);

        MemoryForUsage { types, mask }
    }

    fn compatible(usage: UsageFlags, flags: MemoryPropertyFlags) -> bool {
        if flags.contains(MemoryPropertyFlags::LAZILY_ALLOCATED)
            || flags.contains(MemoryPropertyFlags::PROTECTED)
        {
            false
        } else {
            if usage.intersects(
                UsageFlags::HOST_ACCESS
                    | UsageFlags::UPLOAD
                    | UsageFlags::DOWNLOAD,
            ) {
                flags.contains(MemoryPropertyFlags::HOST_VISIBLE)
            } else {
                true
            }
        }
    }

    fn priority(usage: UsageFlags, flags: MemoryPropertyFlags) -> u32 {
        type Flags = MemoryPropertyFlags;

        let device_local: bool = flags.contains(Flags::DEVICE_LOCAL)
            ^ (usage.is_empty()
                || usage.contains(UsageFlags::FAST_DEVICE_ACCESS));
        let host_visible: bool = flags.contains(Flags::HOST_VISIBLE)
            && !usage.intersects(
                UsageFlags::HOST_ACCESS
                    | UsageFlags::UPLOAD
                    | UsageFlags::DOWNLOAD,
            );
        let cached: bool = flags.contains(Flags::HOST_CACHED)
            ^ usage.contains(UsageFlags::DOWNLOAD);
        let coherent: bool = flags.contains(Flags::HOST_COHERENT)
            ^ (usage.intersects(UsageFlags::UPLOAD | UsageFlags::DOWNLOAD));

        15 - device_local as u32 * 8
            - host_visible as u32 * 4
            - cached as u32 * 2
            - coherent as u32
    }
}

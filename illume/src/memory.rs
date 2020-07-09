bitflags::bitflags! {
    /// Memory usage flags.
    #[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
    pub struct MemoryUsageFlags: u8 {
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

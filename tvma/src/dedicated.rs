use {
    erupt::{
        vk1_0::{self, Vk10DeviceLoaderExt as _},
        vk1_1, DeviceLoader,
    },
    std::{
        fmt::Debug,
        num::NonZeroU64,
        sync::atomic::{AtomicU64, Ordering},
    },
};

/// Memory allocator that creates new memory object for every allocation.
#[derive(Debug, Default)]
pub struct DedicatedAllocator {
    allocated: AtomicU64,

    /// Memory type this allocator allocates from.
    memory_type: u32,
    /// Properties of memory this allcoate allocates.
    flags: vk1_0::MemoryPropertyFlags,
}

impl DedicatedAllocator {
    #[tracing::instrument]
    pub fn new(memory_type: u32, flags: vk1_0::MemoryPropertyFlags) -> Self {
        DedicatedAllocator {
            allocated: AtomicU64::new(0),
            memory_type,
            flags,
        }
    }

    /// Allocates new memory object and returns as memory block.
    /// This function must always me called with same logical device object.
    #[tracing::instrument(skip(device))]
    pub unsafe fn alloc(
        &self,
        device: &DeviceLoader,
        size: u64,
    ) -> Option<DedicatedMemoryBlock> {
        let mut alloc_info = vk1_0::MemoryAllocateInfo::default()
            .builder()
            .allocation_size(size)
            .memory_type_index(self.memory_type);
        let mut flags = vk1_1::MemoryAllocateFlagsInfo::default()
            .builder()
            .flags(vk1_1::MemoryAllocateFlags::DEVICE_ADDRESS);
        flags.extend(&mut *alloc_info);

        match device.allocate_memory(&alloc_info, None, None).result() {
            Ok(memory) => {
                self.allocated.fetch_add(size, Ordering::Relaxed);

                debug_assert_ne!(memory, vk1_0::DeviceMemory::null());
                Some(DedicatedMemoryBlock {
                    memory: NonZeroU64::new_unchecked(memory.0),
                    size,
                    flags: self.flags,
                    memory_type: self.memory_type,
                })
            }
            Err(vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                std::alloc::handle_alloc_error(std::alloc::Layout::new::<()>())
            }
            Err(_) => None,
        }
    }

    #[tracing::instrument(skip(device))]
    pub unsafe fn dealloc(
        &self,
        device: &DeviceLoader,
        block: DedicatedMemoryBlock,
    ) {
        debug_assert_eq!(block.memory_type, self.memory_type);
        debug_assert_eq!(block.flags, self.flags);
        device.free_memory(vk1_0::DeviceMemory(block.memory.get()), None)
    }
}

#[derive(Debug)]
pub struct DedicatedMemoryBlock {
    pub memory: NonZeroU64,
    pub size: u64,
    pub flags: vk1_0::MemoryPropertyFlags,
    pub memory_type: u32,
}

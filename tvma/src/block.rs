use {
    crate::{
        chunked::ChunkedMemoryBlock,
        dedicated::DedicatedMemoryBlock,
        error::{MappingError, OutOfMemory},
        linear::LinearMemoryBlock,
    },
    erupt::{
        vk1_0::{self, DeviceMemory, MemoryMapFlags, MemoryPropertyFlags},
        DeviceLoader,
    },
    std::{convert::TryFrom as _, fmt::Debug, num::NonZeroU64, ptr::NonNull},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    memory: NonZeroU64,
    offset: u64,
    size: u64,
    flags: MemoryPropertyFlags,
    memory_type: u32,
    kind: BlockKind,
}

unsafe impl Send for Block {}
unsafe impl Sync for Block {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    Dedicated,
    Chunked {
        index: u32,
        ptr: Option<NonNull<u8>>,
    },
    Linear {
        index: u64,
        ptr: Option<NonNull<u8>>,
    },
}

impl Block {
    /// Returns block's memory object.
    pub fn memory(&self) -> DeviceMemory {
        DeviceMemory(self.memory.get())
    }

    /// Returns memory property flags of the block's memory.
    pub fn properties(&self) -> MemoryPropertyFlags {
        self.flags
    }

    /// Returns offset of the block in memory object
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns size of the block in memory object
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns end of the block in memory object
    pub fn end(&self) -> u64 {
        self.offset + self.size
    }

    /// Maps buffer memory.
    ///
    /// # Extremely unsafe
    ///
    /// Block must have been allocated from `device`.
    /// Returned pointer will be invalidead after unmapping.
    /// Block may be implicitly unmapped at any point after deallocation.
    /// Returned pointer is not unique. Next call to `map` may return same
    /// pointer and without call to `unmap` **will** return same pointer.
    /// Memory referenced by returned pointer is also shared with `device` and
    /// access should not overlap.
    #[tracing::instrument(skip(device), err)]
    pub unsafe fn map(
        &self,
        device: &DeviceLoader,
        offset: u64,
        size: usize,
    ) -> Result<NonNull<u8>, MappingError> {
        let size =
            u64::try_from(size).map_err(|_| MappingError::OutOfBounds)?;

        if offset.checked_add(size).map_or(true, |end| end > self.size) {
            tracing::error!("Mapping out of bounds");
            return Err(MappingError::OutOfBounds);
        }

        if !self.is_host_visible() {
            Err(MappingError::NonHostVisible)
        } else {
            let mut ptr = std::ptr::null_mut();
            match self.kind {
                BlockKind::Dedicated => {
                    debug_assert_eq!(self.offset, 0);
                    match device
                        .map_memory(
                            DeviceMemory(self.memory.get()),
                            offset,
                            size,
                            Some(MemoryMapFlags::empty()),
                            &mut ptr,
                        )
                        .result()
                    {
                        Ok(()) => Ok(NonNull::new(ptr as *mut u8).unwrap()),
                        Err(vk1_0::Result::ERROR_MEMORY_MAP_FAILED)
                        | Err(vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                            std::alloc::handle_alloc_error(
                                std::alloc::Layout::new::<()>(),
                            )
                        }
                        Err(_) => Err(OutOfMemory.into()),
                    }
                }
                BlockKind::Linear { ptr: Some(ptr), .. }
                | BlockKind::Chunked { ptr: Some(ptr), .. } => {
                    let offset = usize::try_from(offset).unwrap();
                    Ok(
                        // This is safe as `offset` is within allocated memory
                        // range.
                        NonNull::new_unchecked(ptr.as_ptr().add(offset)),
                    )
                }

                BlockKind::Chunked { .. } | BlockKind::Linear { .. } => {
                    debug_assert!(false, "Chunked and Linear block of host-visible memory must be premapped");
                    std::hint::unreachable_unchecked()
                }
            }
        }
    }

    /// Unmaps buffer memory.
    #[tracing::instrument(skip(device))]
    pub unsafe fn unmap(&self, device: &DeviceLoader) {
        if !self.is_host_visible() {
            #[cfg(debug_assertions)]
            panic!("Attempt to unmap non host-visible memory block");
        } else {
            match self.kind {
                BlockKind::Dedicated => {
                    device.unmap_memory(DeviceMemory(self.memory.get()));
                }
                BlockKind::Chunked { ptr, .. }
                | BlockKind::Linear { ptr, .. } => {
                    debug_assert!(ptr.is_some());
                    // Chunked and linear blocks are persistently mapped.
                }
            }
        }
    }

    /// Check if property flags contain `HOST_VISIBLE` flag.
    pub fn is_host_visible(&self) -> bool {
        self.flags.contains(MemoryPropertyFlags::HOST_VISIBLE)
    }

    /// Check if property flags contain `HOST_COHERENT` flag.
    pub fn is_host_coherent(&self) -> bool {
        self.flags.contains(MemoryPropertyFlags::HOST_COHERENT)
    }

    /// Check if property flags contain `HOST_CACHED` flag.
    pub fn is_host_cached(&self) -> bool {
        self.flags.contains(MemoryPropertyFlags::HOST_CACHED)
    }
}

#[derive(Debug)]
pub enum BlockFlavor {
    Dedicated(DedicatedMemoryBlock),
    Linear(LinearMemoryBlock),
    Chunked(ChunkedMemoryBlock),
}

impl From<BlockFlavor> for Block {
    fn from(block: BlockFlavor) -> Block {
        match block {
            BlockFlavor::Dedicated(block) => Block {
                memory: block.memory,
                offset: 0,
                size: block.size,
                flags: block.flags,
                memory_type: block.memory_type,
                kind: BlockKind::Dedicated,
            },
            BlockFlavor::Linear(block) => Block {
                memory: block.memory,
                offset: block.offset,
                size: block.size,
                flags: block.flags,
                memory_type: block.memory_type,
                kind: BlockKind::Linear {
                    index: block.index,
                    ptr: block.ptr,
                },
            },
            BlockFlavor::Chunked(block) => Block {
                memory: block.memory,
                offset: block.offset,
                size: block.size,
                flags: block.flags,
                memory_type: block.memory_type,
                kind: BlockKind::Chunked {
                    index: block.index,
                    ptr: block.ptr,
                },
            },
        }
    }
}

impl From<Block> for BlockFlavor {
    fn from(block: Block) -> BlockFlavor {
        match block.kind {
            BlockKind::Dedicated => {
                assert_eq!(block.offset, 0);
                BlockFlavor::Dedicated(DedicatedMemoryBlock {
                    memory: block.memory,
                    size: block.size,
                    memory_type: block.memory_type,
                    flags: block.flags,
                })
            }
            BlockKind::Linear { index, ptr } => {
                BlockFlavor::Linear(LinearMemoryBlock {
                    memory: block.memory,
                    offset: block.offset,
                    size: block.size,
                    ptr,
                    flags: block.flags,
                    memory_type: block.memory_type,
                    index,
                })
            }
            BlockKind::Chunked { index, ptr } => {
                BlockFlavor::Chunked(ChunkedMemoryBlock {
                    memory: block.memory,
                    offset: block.offset,
                    size: block.size,
                    ptr,
                    flags: block.flags,
                    memory_type: block.memory_type,
                    index,
                })
            }
        }
    }
}

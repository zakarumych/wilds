use {
    crate::align_up,
    erupt::{
        vk1_0::{self, Vk10DeviceLoaderExt as _},
        vk1_1, DeviceLoader,
    },
    std::{convert::TryFrom as _, num::NonZeroU64, ptr::NonNull},
};

/// Chunk with free memory less than this treshold are considered exhausted.
const EXHAUSTED_TRESHOLD: u64 = 1 << 20;

/// Max count of unexhauested chunks that cannot be used
/// for next block allocations.
const UNEXHAUESTED_MAX_COUNT: usize = 4;

#[derive(Debug)]
struct Chunk {
    memory: NonZeroU64,
    offset: u64,
    allocated: u64,
    ptr: Option<NonNull<u8>>,
}

unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}

impl Chunk {
    fn is_exhausted(&self, chunk_size: u64) -> bool {
        if let Some(treshold) = self.offset.checked_add(EXHAUSTED_TRESHOLD) {
            treshold > chunk_size
        } else {
            true
        }
    }

    fn alloc(
        &mut self,
        size: u64,
        align: u64,
        chunk_size: u64,
    ) -> Option<(NonZeroU64, u64, Option<NonNull<u8>>)> {
        let aligned = align_up(align, self.offset)?;
        let end = aligned.checked_add(size)?;
        if end <= chunk_size {
            self.allocated += size;
            self.offset = end;
            let ptr = self.ptr.map(|p| {
                NonNull::new(unsafe { p.as_ptr().add(aligned as usize) })
                    .unwrap()
            });
            Some((self.memory, aligned, ptr))
        } else {
            None
        }
    }

    fn dealloc(&mut self, size: u64) -> bool {
        let left = self.allocated - size;
        self.allocated = left;
        0 == self.allocated
    }
}

#[derive(Debug)]
pub struct LinearAllocator {
    chunk_size: u64,
    memory_type: u32,
    flags: vk1_0::MemoryPropertyFlags,
    offset: u64,
    exhausted: Vec<Option<Chunk>>,
    chunks: Vec<Chunk>,
}

impl LinearAllocator {
    #[tracing::instrument]
    pub fn new(
        chunk_size: u64,
        memory_type: u32,
        flags: vk1_0::MemoryPropertyFlags,
    ) -> Self {
        tracing::info!("New linear allocator created");
        LinearAllocator {
            chunk_size,
            memory_type,
            flags,
            offset: 0,
            exhausted: Vec::new(),
            chunks: Vec::new(),
        }
    }

    pub fn can_allocate(&self, size: u64, align: u64) -> bool {
        size <= self.chunk_size / 2
    }

    fn mappable_memory(&self) -> bool {
        self.flags
            .contains(vk1_0::MemoryPropertyFlags::HOST_VISIBLE)
    }

    #[tracing::instrument(skip(self, device))]
    pub unsafe fn alloc(
        &mut self,
        device: &DeviceLoader,
        size: u64,
        align: u64,
    ) -> Option<LinearMemoryBlock> {
        tracing::trace!("allocating");
        debug_assert!(size < self.chunk_size / 2,
            "Requested size {} is larger than half chunk size {}. This allocation must be handled by DedicatedAllocator",
            size,
            self.chunk_size);

        for (index, chunk) in self.chunks.iter_mut().enumerate() {
            if let Some((memory, offset, ptr)) =
                chunk.alloc(size, align, self.chunk_size)
            {
                let block = LinearMemoryBlock {
                    memory,
                    offset,
                    size,
                    ptr,
                    flags: self.flags,
                    memory_type: self.memory_type,
                    index: index as u64
                        + self.exhausted.len() as u64
                        + self.offset,
                };

                if index == 0 {
                    let exhausted_count = self
                        .chunks
                        .iter()
                        .take_while(|c| c.is_exhausted(self.chunk_size))
                        .count();
                    self.exhausted
                        .extend(self.chunks.drain(..exhausted_count).map(Some));
                }

                return Some(block);
            }
        }

        let mut chunk = self.alloc_chunk(device, self.chunk_size)?;
        let (memory, offset, ptr) =
            chunk.alloc(size, align, self.chunk_size).unwrap();

        let block = LinearMemoryBlock {
            memory,
            offset,
            size,
            ptr,
            flags: self.flags,
            memory_type: self.memory_type,
            index: self.chunks.len() as u64
                + self.exhausted.len() as u64
                + self.offset,
        };

        if self.chunks.len() >= UNEXHAUESTED_MAX_COUNT {
            self.exhausted.extend(
                self.chunks
                    .drain(..self.chunks.len() - UNEXHAUESTED_MAX_COUNT)
                    .map(Some),
            );
        }

        self.chunks.push(chunk);
        Some(block)
    }

    #[tracing::instrument(skip(self, device))]
    pub unsafe fn dealloc(
        &mut self,
        device: &DeviceLoader,
        block: LinearMemoryBlock,
    ) {
        let chunk_freed_error =
            || tracing::error!("Attemtp to dealloc block from freed chunk");

        let index = block.index;

        if index <= self.offset {
            chunk_freed_error();
            return;
        }

        if let Ok(mut index) = usize::try_from(index - self.offset) {
            if index <= self.exhausted.len() {
                match &mut self.exhausted[index] {
                    Some(chunk) => {
                        if chunk.dealloc(block.size) {
                            let chunk = self.exhausted[index].take().unwrap();
                            self.dealloc_chunk(device, chunk);
                        }
                    }
                    None => {
                        chunk_freed_error();
                    }
                }
                let free_count =
                    self.exhausted.iter().take_while(|c| c.is_none()).count();
                self.exhausted.drain(..free_count);
                self.offset += free_count as u64;
                return;
            }
            index -= self.exhausted.len();

            if index <= self.chunks.len() {
                self.chunks[index].dealloc(block.size);
                return;
            }
        }

        tracing::error!(
            "Attempt to dealloc block from not yet allocated chunk"
        );
    }

    #[tracing::instrument(skip(self, device))]
    unsafe fn alloc_chunk(
        &mut self,
        device: &DeviceLoader,
        chunk_size: u64,
    ) -> Option<Chunk> {
        tracing::trace!("Allocating new chunk");
        let mut alloc_info = vk1_0::MemoryAllocateInfo::default()
            .builder()
            .allocation_size(chunk_size)
            .memory_type_index(self.memory_type);
        let mut flags = vk1_1::MemoryAllocateFlagsInfo::default()
            .builder()
            .flags(vk1_1::MemoryAllocateFlags::DEVICE_ADDRESS);
        flags.extend(&mut *alloc_info);

        match device.allocate_memory(&alloc_info, None, None).result() {
            Ok(memory) => {
                tracing::trace!("Chunk memory allocated");

                // Successful allocation.
                debug_assert_ne!(
                    memory,
                    vk1_0::DeviceMemory::null(),
                    "Successful allocation cannot return null memory object"
                );

                let ptr = if self.mappable_memory() {
                    let mut ptr = std::ptr::null_mut();
                    device
                        .map_memory(
                            memory,
                            0,
                            chunk_size,
                            vk1_0::MemoryMapFlags::empty(),
                            &mut ptr,
                        )
                        .result()
                        .map_err(|err| match err {
                            vk1_0::Result::ERROR_MEMORY_MAP_FAILED
                            | vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => {
                                device.free_memory(memory, None);
                                std::alloc::handle_alloc_error(
                                    std::alloc::Layout::new::<()>(),
                                )
                            }
                            _ => {
                                device.free_memory(memory, None);
                            }
                        })
                        .ok()?;

                    debug_assert_ne!(
                        ptr,
                        std::ptr::null_mut(),
                        "Successful memory mapping cannot return null pointer"
                    );

                    Some(NonNull::new_unchecked(ptr as *mut u8))
                } else {
                    None
                };

                Some(Chunk {
                    memory: NonZeroU64::new_unchecked(memory.0),
                    offset: 0,
                    allocated: 0,
                    ptr,
                })
            }
            Err(vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                // Treat host oom as any other code would do.
                std::alloc::handle_alloc_error(std::alloc::Layout::new::<()>())
            }
            Err(vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY) => None,
            Err(err) => {
                tracing::error!(
                    "Unexpected error from `vkAllocateMemory` call: '{}'",
                    err
                );
                None
            }
        }
    }

    #[tracing::instrument(skip(self, device))]
    unsafe fn dealloc_chunk(&mut self, device: &DeviceLoader, chunk: Chunk) {
        if chunk.ptr.is_some() {
            device.unmap_memory(vk1_0::DeviceMemory(chunk.memory.get()));
        }
        device.free_memory(vk1_0::DeviceMemory(chunk.memory.get()), None);
    }
}

#[derive(Debug)]
pub struct LinearMemoryBlock {
    pub memory: NonZeroU64,
    pub offset: u64,
    pub size: u64,
    pub ptr: Option<NonNull<u8>>,
    pub flags: vk1_0::MemoryPropertyFlags,
    pub memory_type: u32,
    pub index: u64,
}

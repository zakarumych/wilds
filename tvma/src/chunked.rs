use {
    crate::align_up,
    erupt::{vk1_0, vk1_1, DeviceLoader, ExtendableFrom as _},
    fastbitset::BoxedBitSet,
    std::{
        cmp::{max, min},
        collections::HashMap,
        convert::TryFrom as _,
        fmt::Debug,
        num::NonZeroU64,
        ptr::NonNull,
    },
};

/// Hierarchical chunk based allocator.
/// This allocator is able to allocate blocks
/// of any size from larger chunks which themselves are allocated
/// from this allocator until very large treshold.
///
/// This limits number of memory objects allocated to very few.
/// This allocator attempts to optimize tradeoff over few key properties:
/// * speed
/// * overhead
/// * reuse
/// * reclaimability - ability to return memory objects back to device.
#[derive(Debug)]
pub struct ChunkedAllocator {
    /// Memory size that is allocated from device.
    device_alloc_treshold: u64,

    /// Any request is rounded up to this value.
    min_block_size: u64,

    /// Memory type this allocator allocates from.
    memory_type: u32,

    /// Properties of memory this allcoate allocates.
    flags: vk1_0::MemoryPropertyFlags,

    /// Dict of size entries.
    sizes: HashMap<u64, Size>,
}

impl ChunkedAllocator {
    #[tracing::instrument]
    pub fn new(
        device_alloc_treshold: u64,
        min_block_size: u64,
        memory_type: u32,
        flags: vk1_0::MemoryPropertyFlags,
    ) -> Self {
        assert!(min_block_size < device_alloc_treshold);
        assert!(
            usize::try_from(device_alloc_treshold).is_ok(),
            "Block size must always fit `usize` when chunk is mapped"
        );

        ChunkedAllocator {
            device_alloc_treshold,
            min_block_size,
            memory_type,
            flags,
            sizes: HashMap::new(),
        }
    }
}

const MIN_CHUNK_LEN: u64 = 8;
const MAX_CHUNK_LEN: u64 = 64;

fn mid_chunk_len(counter: u64) -> u64 {
    max(MIN_CHUNK_LEN, min(MAX_CHUNK_LEN, counter / 2)).next_power_of_two()
}

#[derive(Default)]
struct Size {
    counter: u64,
    unexhausted: BoxedBitSet,
    chunks: slab::Slab<Chunk>,
}

impl std::fmt::Debug for Size {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let alternate = fmt.alternate();
        let mut debug = fmt.debug_struct("Size");
        debug.field("counter", &self.counter);
        if alternate {
            debug.field(
                "chunks",
                &self.chunks.iter().map(|x| x).collect::<Vec<_>>(),
            );
        }
        debug.finish()
    }
}

#[derive(Debug)]
struct Chunk {
    memory: NonZeroU64,
    offset: u64,
    size: u64,
    ptr: Option<NonNull<u8>>,
    index: u32,
    blocks: u64,
}

unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RawBlockAlloc {
    memory: NonZeroU64,
    offset: u64,
    ptr: Option<NonNull<u8>>,
    index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RawBlockDealloc {
    memory: NonZeroU64,
    offset: u64,
    size: u64,
    index: u32,
}

impl Chunk {
    fn len(&self, block_size: u64) -> u64 {
        assert_ne!(block_size, 0);
        self.size / block_size
    }

    #[tracing::instrument(skip(self))]
    unsafe fn alloc(&mut self, block_size: u64) -> (RawBlockAlloc, bool) {
        debug_assert_ne!(self.blocks, 0, "There must be free block");

        debug_assert!(
            self.ptr.map_or(true, |ptr| {
                usize::try_from(self.size).ok().map_or(false, |size| {
                    (ptr.as_ptr() as usize).checked_add(size).is_some()
                })
            }),
            "Pointer to mapping start + chunk size must not overflow"
        );
        let index = self.blocks.trailing_zeros();
        debug_assert!(u64::from(index) < self.len(block_size));
        debug_assert_ne!(self.blocks & (1 << index), 0);
        self.blocks &= !(1 << index);
        let block_offset = block_size * u64::from(index);
        (
            RawBlockAlloc {
                memory: self.memory,
                offset: self.offset + block_offset,
                ptr: self.ptr.map(|ptr| {
                    NonNull::new_unchecked(
                        ptr.as_ptr()
                            .add(usize::try_from(block_offset).unwrap()),
                    )
                }),
                index: 0,
            },
            self.blocks == 0,
        )
    }

    #[tracing::instrument(skip(self))]
    unsafe fn dealloc(&mut self, raw_block: RawBlockDealloc) -> bool {
        let relative_offset = raw_block.offset - self.offset;

        debug_assert_eq!(relative_offset % raw_block.size, 0);
        let index = relative_offset / raw_block.size;
        debug_assert!(index < self.len(raw_block.size));
        debug_assert_eq!(self.blocks & (1 << index), 0);
        self.blocks |= 1 << index;
        self.blocks == !0
    }
}

impl Size {
    #[tracing::instrument(skip(self))]
    unsafe fn alloc(&mut self, block_size: u64) -> Option<RawBlockAlloc> {
        let chunk_index = self.unexhausted.find_set()?;
        let (mut raw_block, exhausted) = self
            .chunks
            .get_mut(chunk_index as usize)
            .unwrap()
            .alloc(block_size);

        if exhausted {
            self.unexhausted.unset(chunk_index);
        }
        self.counter += 1;
        raw_block.index = chunk_index;
        Some(raw_block)
    }

    #[tracing::instrument(skip(self))]
    unsafe fn add_chunk(
        &mut self,
        chunk_block: RawBlockAlloc,
        chunk_size: u64,
        block_size: u64,
    ) -> RawBlockAlloc {
        let len = chunk_size / block_size;
        assert!(len <= MAX_CHUNK_LEN);
        assert!(len >= MIN_CHUNK_LEN);
        assert!(
            self.chunks.len() < BoxedBitSet::UPPER_BOUND as usize,
            "Too many chunks"
        );

        assert!(
            chunk_block.ptr.map_or(true, |ptr| {
                (ptr.as_ptr() as usize).checked_add(usize::try_from(chunk_size).expect("Block size must always fit `usize` when chunk is mapped")).is_some()
            }),
            "Pointer to mapping start + chunk size must not overflow"
        );

        let chunk = Chunk {
            memory: chunk_block.memory,
            offset: chunk_block.offset,
            size: chunk_size,
            ptr: chunk_block.ptr,
            index: chunk_block.index,
            blocks: ((1u128 << len) - 2) as u64, /* FIXME: Make expression
                                                  * cleaner. */
        };

        let index = self.chunks.insert(chunk) as u32;
        self.unexhausted.set(index);

        RawBlockAlloc {
            index,
            ..chunk_block
        }
    }

    #[tracing::instrument(skip(self))]
    unsafe fn dealloc(
        &mut self,
        raw_block: RawBlockDealloc,
    ) -> Option<RawBlockDealloc> {
        let index = raw_block.index;
        debug_assert!(self.chunks.len() > index as usize);
        let chunk = self.chunks.get_unchecked_mut(index as usize);
        if chunk.dealloc(raw_block) {
            self.unexhausted.unset(index);
            let chunk = self.chunks.remove(index as usize);
            Some(RawBlockDealloc {
                memory: chunk.memory,
                offset: chunk.offset,
                size: chunk.size,
                index: chunk.index,
            })
        } else {
            self.unexhausted.set(index);
            None
        }
    }
}

impl ChunkedAllocator {
    pub fn can_allocate(&self, size: u64, align: u64) -> bool {
        match align_up(align, size) {
            Some(size) => size < self.device_alloc_treshold,
            None => false,
        }
    }

    fn mappable_memory(&self) -> bool {
        self.flags
            .contains(vk1_0::MemoryPropertyFlags::HOST_VISIBLE)
    }

    #[tracing::instrument(skip(self, device))]
    unsafe fn alloc_from_new_chunk(
        &mut self,
        device: &DeviceLoader,
        counter: u64,
        block_size: u64,
    ) -> Option<RawBlockAlloc> {
        let min_chunk_size = block_size * MIN_CHUNK_LEN;

        if min_chunk_size >= self.device_alloc_treshold {
            // If even minimal chunk size is bigger than memory object
            // allocation treshold then allocate memory object.
            let chunk = self.alloc_chunk(device, min_chunk_size)?;

            // Add chunk to the size entry.
            // This returns first block of the chunk immediatelly.
            Some(self.sizes.get_mut(&block_size).unwrap().add_chunk(
                chunk,
                min_chunk_size,
                block_size,
            ))
        } else {
            // Findout max chunk size.
            // It cannot exeed memory object allocation treshold.
            let max_chunk_size =
                min(block_size * MAX_CHUNK_LEN, self.device_alloc_treshold);
            // Findout median chunk size to consider first.
            let mid_chunk_size = min(
                block_size * mid_chunk_len(counter),
                self.device_alloc_treshold,
            );
            debug_assert!(max_chunk_size > min_chunk_size);

            // Cycle from mid chunk size to min chunk size and check if there
            // are free blocks
            let mut chunk_size = mid_chunk_size;
            while chunk_size >= min_chunk_size {
                if let Some(chunk_size_entry) = self.sizes.get_mut(&chunk_size)
                {
                    if let Some(raw_block) = chunk_size_entry.alloc(chunk_size)
                    {
                        return Some(
                            self.sizes
                                .get_mut(&block_size)
                                .unwrap()
                                .add_chunk(raw_block, chunk_size, block_size),
                        );
                    }
                }
                chunk_size = chunk_size / 2;
            }

            // Cycle from mid chunk size to max chunk size and check if there
            // are free blocks
            chunk_size = mid_chunk_size;
            while chunk_size <= max_chunk_size {
                if let Some(chunk_size_entry) = self.sizes.get_mut(&chunk_size)
                {
                    if let Some(raw_block) = chunk_size_entry.alloc(chunk_size)
                    {
                        return Some(
                            self.sizes
                                .get_mut(&block_size)
                                .unwrap()
                                .add_chunk(raw_block, chunk_size, block_size),
                        );
                    }
                }
                chunk_size = chunk_size * 2;
            }

            // Cycle from mid chunk size to min chunk size and allocate chunk
            // for first size entry initialized
            chunk_size = mid_chunk_size;
            while chunk_size >= min_chunk_size {
                if let Some(chunk_size_entry) = self.sizes.get(&chunk_size) {
                    let counter = chunk_size_entry.counter;
                    if let Some(raw_block) =
                        self.alloc_from_new_chunk(device, counter, chunk_size)
                    {
                        return Some(
                            self.sizes
                                .get_mut(&block_size)
                                .unwrap()
                                .add_chunk(raw_block, chunk_size, block_size),
                        );
                    }
                }
                chunk_size = chunk_size / 2;
            }

            // Cycle from mid chunk size to max chunk size and allocate chunk
            // for first size entry initialized
            chunk_size = mid_chunk_size;
            while chunk_size <= max_chunk_size {
                if let Some(chunk_size_entry) = self.sizes.get(&chunk_size) {
                    let counter = chunk_size_entry.counter;
                    if let Some(raw_block) =
                        self.alloc_from_new_chunk(device, counter, chunk_size)
                    {
                        return Some(
                            self.sizes
                                .get_mut(&block_size)
                                .unwrap()
                                .add_chunk(raw_block, chunk_size, block_size),
                        );
                    }
                }
                chunk_size = chunk_size * 2;
            }

            chunk_size = max_chunk_size;
            let chunk_size_entry = self
                .sizes
                .entry(chunk_size)
                .or_insert_with(|| Size::default());
            let counter = chunk_size_entry.counter;
            let raw_block =
                self.alloc_from_new_chunk(device, counter, chunk_size)?;
            Some(
                self.sizes
                    .get_mut(&block_size)
                    .unwrap()
                    .add_chunk(raw_block, chunk_size, block_size),
            )
        }
    }

    #[tracing::instrument(level = "info", skip(self, device))]
    unsafe fn alloc_chunk(
        &mut self,
        device: &DeviceLoader,
        chunk_size: u64,
    ) -> Option<RawBlockAlloc> {
        let mut alloc_info = vk1_0::MemoryAllocateInfoBuilder::new()
            .allocation_size(chunk_size)
            .memory_type_index(self.memory_type);
        let mut flags = vk1_1::MemoryAllocateFlagsInfoBuilder::new()
            .flags(vk1_1::MemoryAllocateFlags::DEVICE_ADDRESS);
        alloc_info = alloc_info.extend_from(&mut flags);

        match device.allocate_memory(&alloc_info, None, None).result() {
            Ok(memory) => {
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
                            Some(vk1_0::MemoryMapFlags::empty()),
                            &mut ptr,
                        )
                        .result()
                        .map_err(|err| match err {
                            vk1_0::Result::ERROR_MEMORY_MAP_FAILED
                            | vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => {
                                device.free_memory(Some(memory), None);
                                std::alloc::handle_alloc_error(
                                    std::alloc::Layout::new::<()>(),
                                )
                            }
                            _ => {
                                device.free_memory(Some(memory), None);
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

                Some(RawBlockAlloc {
                    memory: NonZeroU64::new_unchecked(memory.0),
                    offset: 0,
                    ptr,
                    index: 0,
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
    pub unsafe fn alloc(
        &mut self,
        device: &DeviceLoader,
        size: u64,
        align: u64,
    ) -> Option<ChunkedMemoryBlock> {
        let aligned_size = align_up(align, size)?;
        debug_assert!(aligned_size < self.device_alloc_treshold,
            "Requested size with alignment {} is larger than treshold size {}. This allocation must be handled by DedicatedAllocator",
            aligned_size,
            self.device_alloc_treshold);

        let size_entry = self
            .sizes
            .entry(aligned_size)
            .or_insert_with(|| Size::default());

        let raw_block = match size_entry.alloc(aligned_size) {
            Some(raw_block) => raw_block,
            None => {
                let counter = size_entry.counter;
                self.alloc_from_new_chunk(device, counter, aligned_size)?
            }
        };

        Some(ChunkedMemoryBlock {
            memory: raw_block.memory,
            offset: raw_block.offset,
            size: aligned_size,
            ptr: raw_block.ptr,
            memory_type: self.memory_type,
            flags: self.flags,
            index: raw_block.index,
        })
    }

    #[tracing::instrument(skip(self, device))]
    unsafe fn dealloc_raw(
        &mut self,
        device: &DeviceLoader,
        raw_block: RawBlockDealloc,
    ) {
        let mut raw_block_opt = Some(raw_block);
        while let Some(raw_block) = raw_block_opt.take() {
            let size_entry = self
                .sizes
                .get_mut(&raw_block.size)
                .expect("Size entry where block was allocated must exist");

            let chunk_block_opt = size_entry.dealloc(raw_block);

            match chunk_block_opt {
                None => {}
                Some(chunk_block)
                    if chunk_block.size >= self.device_alloc_treshold =>
                {
                    // Note: memory is implicitly unmapped.
                    device.free_memory(
                        Some(vk1_0::DeviceMemory(chunk_block.memory.get())),
                        None,
                    );
                }
                Some(chunk_block) => {
                    raw_block_opt = Some(chunk_block);
                }
            }
        }
    }

    #[tracing::instrument(skip(self, device))]
    pub unsafe fn dealloc(
        &mut self,
        device: &DeviceLoader,
        block: ChunkedMemoryBlock,
    ) {
        self.dealloc_raw(
            device,
            RawBlockDealloc {
                memory: block.memory,
                offset: block.offset,
                size: block.size,
                index: block.index,
            },
        )
    }
}

#[derive(Debug)]
pub struct ChunkedMemoryBlock {
    pub memory: NonZeroU64,
    pub offset: u64,
    pub size: u64,
    pub ptr: Option<NonNull<u8>>,
    pub flags: vk1_0::MemoryPropertyFlags,
    pub memory_type: u32,
    pub index: u32,
}

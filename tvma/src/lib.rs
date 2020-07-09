//! This crate aims to be minimal implementation of vulkan device memory
//! allocator.
//!
//! It doesn't involve any complex and advanced techniques but allows very
//! simple usage.
//!
//! It uses `erupt` crate as rust-y Vulkan API

mod block;
mod chunked;
mod dedicated;
mod error;
mod linear;
mod usage;

use {
    self::{
        block::BlockFlavor, chunked::*, dedicated::*, linear::*,
        usage::MemoryForUsage,
    },
    erupt::{vk1_0, DeviceLoader},
    parking_lot::Mutex,
    std::{
        convert::TryInto as _,
        fmt::Debug,
        sync::atomic::{AtomicU64, Ordering},
    },
    tinyvec::ArrayVec,
};

pub use self::{block::Block, error::*, usage::UsageFlags};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Config {
    /// Size at which dedicated allocation is performed when prefered.
    /// If greater than `dedicated_treshold_high` then dedicated preference
    /// would be ignored.
    pub dedicated_treshold_low: u64,

    /// Size at which dedicated allocation is always performed.
    pub dedicated_treshold_high: u64,

    /// Allocation size for linear allocation.
    /// Used for `Upload` and `Download` usage types.
    /// If required size is greater than this value - dedicated allocation is
    /// performed even if smaller than `dedicated_treshold_high`.
    pub line_size: u64,

    /// Minimal size of blocks in chunked allocator.
    /// Any smaller request will be bumped to this value when chunked strategy
    /// is used.
    pub min_chunk_block: u64,
}

#[derive(Debug)]
pub struct Allocator {
    dedicated_treshold_low: u64,
    dedicated_treshold_high: u64,

    /// Maps `UsageFlags` (as integer) to memory types in prioritised order.
    memory_for_usage: [MemoryForUsage; 32],

    dedicated: Vec<DedicatedAllocator>,
    linear: Vec<Mutex<LinearAllocator>>,
    chunked: Vec<Mutex<ChunkedAllocator>>,

    /// Indices of heaps for memory types.
    type_to_pool: ArrayVec<[u32; 32]>,

    /// Memory poold information.
    heaps: ArrayVec<[Heap; 32]>,
}

/// Request for dedicated memory block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Dedicated {
    Indifferent,
    Preferred,
    Required,
}

impl Default for Dedicated {
    fn default() -> Self {
        Dedicated::Indifferent
    }
}

impl Allocator {
    #[tracing::instrument]
    pub fn new(
        config: Config,
        properties: &vk1_0::PhysicalDeviceMemoryProperties,
    ) -> Self {
        assert!(
            config.dedicated_treshold_low <= config.dedicated_treshold_high
        );
        assert!(config.line_size >= config.dedicated_treshold_high);
        assert!(config.min_chunk_block < config.dedicated_treshold_high);

        let memory_types =
            &properties.memory_types[..properties.memory_type_count as usize];
        let memory_heaps =
            &properties.memory_heaps[..properties.memory_heap_count as usize];

        let memory_for_usage = (0..32)
            .map(|bits| {
                let flags = UsageFlags::from_bits(bits).unwrap();
                MemoryForUsage::for_usage(flags, memory_types)
            })
            .collect::<Vec<_>>();

        let type_to_pool = memory_types
            .iter()
            .map(|memory_type| memory_type.heap_index)
            .collect();

        let heaps = memory_heaps
            .iter()
            .map(|heap| Heap {
                size: heap.size,
                used: AtomicU64::new(0),
            })
            .collect::<ArrayVec<[_; 32]>>();

        Allocator {
            dedicated_treshold_low: config.dedicated_treshold_low,
            dedicated_treshold_high: config.dedicated_treshold_high,
            memory_for_usage: memory_for_usage[..].try_into().unwrap(),
            dedicated: memory_types
                .iter()
                .enumerate()
                .map(|(index, memory_type)| {
                    DedicatedAllocator::new(
                        index as u32,
                        memory_type.property_flags,
                    )
                })
                .collect(),
            chunked: memory_types
                .iter()
                .enumerate()
                .map(|(index, memory_type)| {
                    let heap_size = heaps[memory_type.heap_index as usize].size;
                    Mutex::new(ChunkedAllocator::new(
                        config.dedicated_treshold_high.min(heap_size / 32),
                        config.min_chunk_block,
                        index as u32,
                        memory_type.property_flags,
                    ))
                })
                .collect(),

            linear: memory_types
                .iter()
                .enumerate()
                .map(|(index, memory_type)| {
                    let heap_size =
                        memory_heaps[memory_type.heap_index as usize].size;
                    Mutex::new(LinearAllocator::new(
                        config.line_size.min(heap_size / 32),
                        index as u32,
                        memory_type.property_flags,
                    ))
                })
                .collect(),

            type_to_pool,
            heaps,
        }
    }

    /// Allocate new memory block.
    /// If successful returns newly allocated block which is
    /// * has size not smaller than `size`
    /// * aligned at least to `align`
    /// * supports specified `usage` pattern
    /// * allocated from one of specified `memory_types`
    /// * uses `dedicated` memory object if requires or prefers and is not too
    ///   small.
    ///
    /// # Safety
    ///
    /// Each `Allocator` instance must always be used with same `device`.
    #[tracing::instrument(skip(self, device))]
    pub unsafe fn alloc(
        &self,
        device: &DeviceLoader,
        size: u64,
        align: u64,
        memory_types: u32,
        usage: UsageFlags,
        dedicated: Dedicated,
    ) -> Result<Block, Error> {
        if 0 == self.memory_for_usage[usage.bits() as usize].mask()
            & memory_types
        {
            return Err(NoCompatibleMemory.into());
        }

        let strategy = match dedicated {
            Dedicated::Required => Strategy::Dedicated,
            Dedicated::Preferred if size >= self.dedicated_treshold_low => {
                Strategy::Dedicated
            }
            _ if size >= self.dedicated_treshold_high => Strategy::Dedicated,
            // _ => Strategy::Chunked,
            _ => Strategy::Linear,
        };

        for memory_type in self.memory_for_usage[usage.bits() as usize]
            .types()
            .iter()
            .copied()
            .filter(|&m| memory_types & (1 << m) != 0)
        {
            // Check if pool has memory
            let pool =
                &self.heaps[self.type_to_pool[memory_type as usize] as usize];
            if pool.can_allocate(size) {
                let result = match strategy {
                    Strategy::Dedicated => self.dedicated[memory_type as usize]
                        .alloc(device, size)
                        .map(BlockFlavor::Dedicated),
                    Strategy::Linear => self.linear[memory_type as usize]
                        .lock()
                        .alloc(device, size, align)
                        .map(BlockFlavor::Linear),
                    Strategy::Chunked => self.chunked[memory_type as usize]
                        .lock()
                        .alloc(device, size, align)
                        .map(BlockFlavor::Chunked),
                };
                match result {
                    Some(block) => {
                        pool.mark_allocated(size);
                        return Ok(block.into());
                    }
                    None => continue,
                }
            }
        }
        Err(OutOfMemory.into())
    }

    #[tracing::instrument(skip(self, device))]
    pub unsafe fn dealloc(&self, device: &DeviceLoader, block: Block) {
        match BlockFlavor::from(block) {
            BlockFlavor::Dedicated(block) => {
                debug_assert!(
                    (block.memory_type as usize) < self.dedicated.len()
                );
                self.dedicated
                    .get_unchecked(block.memory_type as usize)
                    .dealloc(device, block)
            }
            BlockFlavor::Chunked(block) => {
                debug_assert!(
                    (block.memory_type as usize) < self.chunked.len()
                );
                self.chunked
                    .get_unchecked(block.memory_type as usize)
                    .lock()
                    .dealloc(device, block)
            }
            BlockFlavor::Linear(block) => {
                debug_assert!(
                    (block.memory_type as usize) < self.chunked.len()
                );
                self.linear
                    .get_unchecked(block.memory_type as usize)
                    .lock()
                    .dealloc(device, block)
            }
        }
    }
}

enum Strategy {
    Dedicated,
    Linear,

    #[allow(dead_code)]
    Chunked,
}

#[derive(Debug, Default)]
struct Heap {
    used: AtomicU64,
    size: u64,
}

impl Heap {
    /// Checks is there is memory left in pool.
    /// Even if this function returned `true` the pool may be exhausted
    /// by unaccounted allocations.
    /// But attempt to allocate memory from this pool after this function
    /// returned `false` would propably fail.
    fn can_allocate(&self, size: u64) -> bool {
        self.used
            .load(Ordering::Relaxed)
            .checked_add(size)
            .map_or(false, |used| used <= self.size)
    }

    /// Marks that more memory was allocated from this pool.
    fn mark_allocated(&self, size: u64) {
        self.used.fetch_add(size, Ordering::Relaxed);
    }
}

fn align_up(align_mask: u64, value: u64) -> Option<u64> {
    Some(value.checked_add(align_mask)? & !align_mask)
}

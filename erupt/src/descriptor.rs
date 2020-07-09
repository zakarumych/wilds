use erupt::vk1_0;
use illume::{DescriptorSetLayoutBinding, DescriptorType};
use std::{
    hash::{Hash, Hasher},
    ops::Deref,
};

const DESCRIPTOR_TYPES_COUNT: usize = 12;

fn descriptor_type_from_index(index: usize) -> vk1_0::DescriptorType {
    debug_assert!(index < DESCRIPTOR_TYPES_COUNT);

    match index {
        0 => {
            debug_assert_eq!(DescriptorType::Sampler as usize, index);

            vk1_0::DescriptorType::SAMPLER
        }
        1 => {
            debug_assert_eq!(
                DescriptorType::CombinedImageSampler as usize,
                index
            );

            vk1_0::DescriptorType::COMBINED_IMAGE_SAMPLER
        }
        2 => {
            debug_assert_eq!(DescriptorType::SampledImage as usize, index);

            vk1_0::DescriptorType::SAMPLED_IMAGE
        }
        3 => {
            debug_assert_eq!(DescriptorType::StorageImage as usize, index);

            vk1_0::DescriptorType::STORAGE_IMAGE
        }
        4 => {
            debug_assert_eq!(
                DescriptorType::UniformTexelBuffer as usize,
                index
            );

            vk1_0::DescriptorType::UNIFORM_TEXEL_BUFFER
        }
        5 => {
            debug_assert_eq!(
                DescriptorType::StorageTexelBuffer as usize,
                index
            );

            vk1_0::DescriptorType::STORAGE_TEXEL_BUFFER
        }
        6 => {
            debug_assert_eq!(DescriptorType::UniformBuffer as usize, index);

            vk1_0::DescriptorType::UNIFORM_BUFFER
        }
        7 => {
            debug_assert_eq!(DescriptorType::StorageBuffer as usize, index);

            vk1_0::DescriptorType::STORAGE_BUFFER
        }
        8 => {
            debug_assert_eq!(
                DescriptorType::UniformBufferDynamic as usize,
                index
            );

            vk1_0::DescriptorType::UNIFORM_BUFFER_DYNAMIC
        }
        9 => {
            debug_assert_eq!(
                DescriptorType::StorageBufferDynamic as usize,
                index
            );

            vk1_0::DescriptorType::STORAGE_BUFFER_DYNAMIC
        }
        10 => {
            debug_assert_eq!(DescriptorType::InputAttachment as usize, index);

            vk1_0::DescriptorType::INPUT_ATTACHMENT
        }
        11 => {
            debug_assert_eq!(
                DescriptorType::AccelerationStructure as usize,
                index
            );

            vk1_0::DescriptorType::ACCELERATION_STRUCTURE_KHR
        }
        _ => unreachable!(),
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorSizesBuilder {
    sizes: [u32; DESCRIPTOR_TYPES_COUNT],
}

impl DescriptorSizesBuilder {
    /// Create new instance without descriptors.
    pub fn zero() -> Self {
        DescriptorSizesBuilder {
            sizes: [0; DESCRIPTOR_TYPES_COUNT],
        }
    }

    /// Add a single layout binding.
    /// Useful when created with `DescriptorSizes::zero()`.
    pub fn add_binding(&mut self, binding: &DescriptorSetLayoutBinding) {
        self.sizes[binding.ty as usize] += binding.count;
    }

    /// Calculate ranges from bindings.
    pub fn from_bindings(bindings: &[DescriptorSetLayoutBinding]) -> Self {
        let mut ranges = Self::zero();

        for binding in bindings {
            ranges.add_binding(binding);
        }

        ranges
    }

    pub fn build(self) -> DescriptorSizes {
        let mut sizes = [vk1_0::DescriptorPoolSize::default()
            .builder()
            ._type(vk1_0::DescriptorType::SAMPLER)
            .descriptor_count(0);
            DESCRIPTOR_TYPES_COUNT];

        let mut count = 0u8;

        for (i, size) in self.sizes.iter().copied().enumerate() {
            if size > 0 {
                sizes[count as usize]._type = descriptor_type_from_index(i);

                sizes[count as usize].descriptor_count = size;

                count += 1;
            }
        }

        DescriptorSizes { sizes, count }
    }
}

/// Number of descriptors per type.
#[derive(Clone, Debug)]
pub struct DescriptorSizes {
    sizes: [vk1_0::DescriptorPoolSizeBuilder<'static>; DESCRIPTOR_TYPES_COUNT],
    count: u8,
}

impl DescriptorSizes {
    pub fn as_slice(&self) -> &[vk1_0::DescriptorPoolSizeBuilder<'static>] {
        &self.sizes[..self.count.into()]
    }

    /// Calculate ranges from bindings.
    pub fn from_bindings(bindings: &[DescriptorSetLayoutBinding]) -> Self {
        DescriptorSizesBuilder::from_bindings(bindings).build()
    }
}

impl Deref for DescriptorSizes {
    type Target = [vk1_0::DescriptorPoolSizeBuilder<'static>];

    fn deref(&self) -> &[vk1_0::DescriptorPoolSizeBuilder<'static>] {
        self.as_slice()
    }
}

impl Hash for DescriptorSizes {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        for size in self.as_slice() {
            hasher.write_u32(size.descriptor_count);
        }
    }
}

impl PartialEq for DescriptorSizes {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_slice().iter().zip(rhs.as_slice()).all(|(l, r)| {
            l._type == r._type && l.descriptor_count == r.descriptor_count
        })
    }
}

impl Eq for DescriptorSizes {}

// impl Add for DescriptorSizes {
//     type Output = Self;
//     fn add(mut self, rhs: Self) -> Self {
//         self += rhs;
//         self
//     }
// }

// impl AddAssign for DescriptorSizes {
//     fn add_assign(&mut self, rhs: Self) {
//         for i in 0..DESCRIPTOR_TYPES_COUNT {
//             self.sizes[i].descriptor_count += rhs.sizes[i].descriptor_count;
//         }
//     }
// }

// impl Sub for DescriptorSizes {
//     type Output = Self;
//     fn sub(mut self, rhs: Self) -> Self {
//         self -= rhs;
//         self
//     }
// }

// impl SubAssign for DescriptorSizes {
//     fn sub_assign(&mut self, rhs: Self) {
//         for i in 0..DESCRIPTOR_TYPES_COUNT {
//             self.sizes[i].descriptor_count -= rhs.sizes[i].descriptor_count;
//         }
//     }
// }

// impl Mul<u32> for DescriptorSizes {
//     type Output = Self;
//     fn mul(mut self, rhs: u32) -> Self {
//         self *= rhs;
//         self
//     }
// }

// impl MulAssign<u32> for DescriptorSizes {
//     fn mul_assign(&mut self, rhs: u32) {
//         for i in 0..DESCRIPTOR_TYPES_COUNT {
//             self.sizes[i].descriptor_count *= rhs;
//         }
//     }
// }

// #[derive(Debug)]
// struct Allocation {
//     sets: Vec<DescriptorSet>,
// }

// #[derive(Debug)]
// struct DescriptorPool {
//     raw: vk1_0::DescriptorPool,
//     size: u32,
//     free: u32,
// }

// #[derive(Debug)]
// struct DescriptorBucket {
//     pools: Slab<DescriptorPool>,
//     total: u64,
// }

// impl DescriptorBucket {
//     fn new() -> Self {
//         DescriptorBucket {
//             pools: Slab::new(),
//             total: 0,
//         }
//     }

//     fn new_pool_size(&self, count: u32) -> u32 {
//         MIN_SETS // at least MIN_SETS
//             .max(count) // at least enough for allocation
//             .max(self.total.min(MAX_SETS as u64) as u32) // at least as much
// as was allocated so far capped to MAX_SETS             .next_power_of_two()
// // rounded up to nearest 2^N     }

//     unsafe fn allocate(
//         &mut self,
//         device: &EruptDevice,
//         mut layouts: &[DescriptorSetLayout],
//         sizes: &DescriptorSizes,
//         allocation: &mut Allocation,
//     ) -> Result<(), OutOfMemory> {
//         if layouts.len() == 0 {
//             return Ok(());
//         }

//         if u32::try_from(layouts.len()).is_err() {
//             return Err(OutOfMemory);
//         }

//         for (index, pool) in self.pools.iter_mut() {
//             if pool.free == 0 {
//                 continue;
//             }

//             let allocate = pool.free.min(layouts.len() as u32);
//             tracing::trace!("Allocate {} from exising pool", allocate);

//             let mut sets = device
//                 .allocate_descriptor_sets(
//                     &vk1_0::DescriptorSetAllocateInfo::default().builder()
//                         .descriptor_pool(pool.raw)
//                         .set_layouts(
//                             &layouts[..allocate as usize]
//                                 .iter()
//                                 .map(|l| {
//
// debug_assert_eq!(l.erupt_ref(device).sizes, sizes);
// l.erupt_ref(device).handle                                 })
//                                 .collect::<SmallVec<[_; 16]>>(),
//                         ),
//                 )
//                 .map_err(|err| match err {
//                     vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
//                     vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY =>
// out_of_host_memory(),
// vk1_0::Result::ERROR_FRAGMENTED_POOL => panic!("Pool fragmentation is
// unxepected"), vk1_0::Result::ERROR_OUT_OF_POOL_MEMORY => {
// panic!("Exahauseted pool is unexpected")                     }
//                 })?;

//             // FIXME Avoid double allocation.
//             allocation.sets.extend(sets.into_iter().map(|handle| {
//                 DescriptorSet::make(EruptDescriptorSet {
//                     handle,
//                     owner: Arc::downgrade(device),
//                 })
//             }));

//             layouts = &layouts[allocate as usize..];
//             pool.free -= allocate;
//             self.total += allocate as u64;

//             if layouts.is_empty() {
//                 return Ok(());
//             }
//         }

//         while !layouts.is_empty() {
//             let size = self.new_pool_size(layouts.len() as u32);
//             let pool_sizes = sizes * size;
//             tracing::trace!(
//                 "Create new pool with {} sets and {:?} descriptors",
//                 size,
//                 pool_sizes,
//             );
//             let raw = device
//                 .create_descriptor_pool(
//                     &vk1_0::DescriptorPoolCreateInfo::default().builder()
//                         .max_sets(size)
//                         .pool_sizes(&*pool_sizes),
//                     None,
//                 )
//                 .map_err(oom_error_from_erupt)?;

//             let allocate = size.min(layouts.len() as u32);

//             let index = self.pools.insert(DescriptorPool {
//                 raw,
//                 size,
//                 free: size,
//             });
//             let index = self.pools.len() - 1;
//             let pool = &mut self.pools[index];

//             let mut sets = device
//                 .logical
//                 .allocate_descriptor_sets(
//                     &vk1_0::DescriptorSetAllocateInfo::default().builder()
//                         .descriptor_pool(pool.raw)
//                         .set_layouts(&layouts[..allocate as usize]),
//                 )
//                 .map_err(|err| match err {
//                     vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
//                     vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY =>
// out_of_host_memory(),
// vk1_0::Result::ERROR_FRAGMENTED_POOL => panic!("Pool fragmentation is
// unxepected"), vk1_0::Result::ERROR_OUT_OF_POOL_MEMORY => {
// panic!("Exahauseted pool is unexpected")                     }
//                 })?;

//             // FIXME Avoid double allocation.
//             allocation
//                 .sets
//                 .extend(sets.into_iter().map(|set| (set, index)));

//             layouts = &layouts[allocate as usize..];
//             pool.free -= allocate;
//             self.total += allocate as u64;
//         }

//         Ok(())
//     }

//     unsafe fn free(
//         &mut self,
//         device: &EruptDevice,
//         sets: impl IntoIterator<Item = (vk1_0::DescriptorSet, usize)>,
//     ) {
//         let freed = sets.into_iter().fold(0, |acc, (set, pool)| {
//             let pool = &mut self.pools[pool];
//             device.free_descriptor_sets(pool.raw, &[set]);
//             pool.free += 1;
//             acc + 1
//         });

//         self.total -= freed;
//         tracing::trace!("Freed {} sets from descriptor bucket", freed);
//     }

//     unsafe fn cleanup(&mut self, device: &EruptDevice) {
//         let mut one_full = false;
//         for (index, pool) in self.pools.iter_mut() {
//             if pool.free == pool.size {
//                 if std::mem::replace(&mut one_full, true) {
//                     device.destroy_descriptor_pool(pool.raw, None);
//                 }
//             }
//         }
//     }
// }

// /// Descriptor allocator.
// /// Can be used to allocate descriptor sets for any layout.
// #[derive(Debug)]
// pub struct DescriptorAllocator {
//     buckets: HashMap<DescriptorSizes, DescriptorBucket>,
//     allocation: Allocation,
//     total: u64,
// }

// impl DescriptorAllocator {
//     /// Create new allocator instance.
//     pub fn new() -> Self {
//         DescriptorAllocator {
//             buckets: HashMap::new(),
//             allocation: Allocation {
//                 sets: SmallVec::new(),
//             },
//             total: 0,
//         }
//     }

//     /// Allocate descriptor set with specified layout.
//     /// `DescriptorSizes` must match descriptor numbers of the layout.
//     /// `DescriptorSizes` can be constructed [from bindings] that were used
//     /// to create layout instance.
//     ///
//     /// [from bindings]: .
//     pub unsafe fn allocate(
//         &mut self,
//         device: &EruptDevice,
//         layouts: &[DescriptorSetLayout],
//         count: u32,
//         extend: &mut impl Extend<DescriptorSet>,
//     ) -> Result<(), OutOfMemory> {
//         if count == 0 {
//             return Ok(());
//         }

//         tracing::trace!("Allocating {} sets with layout {:?}", count,
// layouts);

//         let mut last: Option<&DescriptorSizes> = None;
//         let mut similar_layouts = SmallVec::<[_; 16]>::new();

//         for layout in layouts.iter() {
//             let layout = layout.erupt_ref(device);
//             if let Some(last) = last {
//                 if *last != layout.sizes {
//                     let bucket = self
//                         .buckets
//                         .entry(*last)
//                         .or_insert_with(|| DescriptorBucket::new());

//                     match bucket.allocate(
//                         &device.logical,
//                         &similar_layouts,
//                         last,
//                         &mut self.allocation,
//                     ) {
//                         Ok(()) => {}
//                         Err(OutOfMemory) => {
//                             return Err(OutOfMemory);
//                         }
//                     }
//                 }
//             } else {
//                 last = Some(layout.sizes);
//                 similar_layouts.reset();
//                 similar_layouts.push(layout.handle);
//             }
//         }

//         Ok(())
//     }

//     /// Free descriptor sets.
//     ///
//     /// # Safety
//     ///
//     /// None of descriptor sets can be referenced in any pending command
// buffers.     /// All command buffers where at least one of descriptor sets
// referenced     /// move to invalid state.
//     pub unsafe fn free(&mut self, all_sets: impl IntoIterator<Item =
// DescriptorSet>) {         let mut free: Option<(DescriptorSizes, u64,
// SmallVec<[DescriptorSet; 32]>)> = None;

//         // Collect contig
//         for set in all_sets {
//             match &mut free {
//                 slot @ None => {
//                     slot.replace((set.ranges, set.pool, smallvec![set.raw]));
//                 }
//                 Some((ranges, pool, raw_sets)) if *ranges == set.ranges &&
// *pool == set.pool => {                     raw_sets.push(set.raw);
//                 }
//                 Some((ranges, pool, raw_sets)) => {
//                     let bucket = self
//                         .buckets
//                         .get_mut(ranges)
//                         .expect("Set should be allocated from this
// allocator");                     debug_assert!(bucket.total >= raw_sets.len()
// as u64);

//                     bucket.free(raw_sets.drain(..), *pool);
//                     *pool = set.pool;
//                     *ranges = set.ranges;
//                     raw_sets.push(set.raw);
//                 }
//             }
//         }

//         if let Some((ranges, pool, raw_sets)) = free {
//             let bucket = self
//                 .buckets
//                 .get_mut(&ranges)
//                 .expect("Set should be allocated from this allocator");
//             debug_assert!(bucket.total >= raw_sets.len() as u64);

//             bucket.free(raw_sets, pool);
//         }
//     }

//     /// Perform cleanup to allow resources reuse.
//     pub unsafe fn cleanup(&mut self, device: &EruptDevice) {
//         self.buckets
//             .values_mut()
//             .for_each(|bucket| bucket.cleanup(device));
//     }
// }

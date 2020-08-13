#![deny(non_snake_case)]
#![deny(unreachable_patterns)]
#![deny(unused_unsafe)]
#![deny(missing_copy_implementations)]
#![deny(missing_debug_implementations)]
#![deny(unused_must_use)]
// #![deny(unused_variables)]
#![allow(unused_imports)]

use std::{
    cmp::{Ord, Ordering, PartialOrd},
    convert::{TryFrom as _, TryInto as _},
    error::Error,
    fmt::Debug,
    num::TryFromIntError,
};

macro_rules! define_handle {
    ($(#[$meta:meta])* $vis:vis struct $handle:ident($handle_info:ident);) => {
        $(#[$meta])*
        #[derive(Clone, Hash, PartialEq, Eq)]
        #[repr(transparent)]
        $vis struct $handle {
            handle: Handle<Self>,
        }

        impl std::fmt::Debug for $handle {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.handle, fmt)
            }
        }

        impl $handle {
            #[doc("Returns `")]
            #[doc($handle_info)]
            #[doc("` that was used when resource was created.")]
            #[doc = "This information cannot be modified during resource lifetime."]
            $vis fn info(&self) -> &$handle_info {
                self.handle.info()
            }
        }

        impl ResourceTrait for $handle {
            type Info = $handle_info;

            fn from_handle(handle: Handle<Self>) -> Self {
                Self { handle }
            }

            fn handle(&self) -> &Handle<Self> {
                &self.handle
            }
        }
    };
}

mod buffer;
mod command;
mod descriptor;
mod device;
mod fence;
mod format;
mod graphics;
mod image;
mod memory;
mod physical;
mod pipeline;
mod queue;
mod render_pass;
mod resource;
mod sampler;
mod semaphore;
mod shader;
mod stage;
mod surface;

pub use self::{
    buffer::*, command::*, descriptor::*, device::*, fence::*, format::*,
    graphics::*, image::*, memory::*, physical::*, pipeline::*, queue::*,
    render_pass::*, resource::*, sampler::*, semaphore::*, shader::*, stage::*,
    surface::*,
};

/// Image size is defiend to `u32` which is standard for graphics API today.
pub type ImageSize = u32;

/// Two dimensional extent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Extent2d {
    /// Width of the extent.
    pub width: ImageSize,

    /// Height of the extent.
    pub height: ImageSize,
}

impl PartialOrd for Extent2d {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let width = Ord::cmp(&self.width, &other.width);

        let height = Ord::cmp(&self.height, &other.height);

        merge_ordering(width, height)
    }
}

impl Extent2d {
    pub fn into_3d(self) -> Extent3d {
        Extent3d {
            width: self.width,
            height: self.height,
            depth: 1,
        }
    }
}

/// Three dimensional extent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Extent3d {
    /// Width of the extent.
    pub width: ImageSize,

    /// Height of the extent.
    pub height: ImageSize,

    /// Depth of the extent.
    pub depth: ImageSize,
}

impl PartialOrd for Extent3d {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let width = Ord::cmp(&self.width, &other.width);
        let height = Ord::cmp(&self.height, &other.height);
        let depth = Ord::cmp(&self.depth, &other.depth);

        merge_ordering(merge_ordering(width, height)?, depth)
    }
}

impl Extent3d {
    pub fn into_2d(self) -> Extent2d {
        Extent2d {
            width: self.width,
            height: self.height,
        }
    }
}

/// Image offset is defiend to `i32` which is standard for graphics API today.
pub type ImageOffset = i32;

/// Two dimensional offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Offset2d {
    /// Width offset.
    pub x: ImageOffset,

    /// Height offset.
    pub y: ImageOffset,
}

impl Offset2d {
    pub const ZERO: Self = Offset2d { x: 0, y: 0 };

    pub fn from_extent(extent: Extent2d) -> Result<Self, TryFromIntError> {
        Ok(Offset2d {
            x: extent.width.try_into()?,
            y: extent.height.try_into()?,
        })
    }
}

/// Three dimensional offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Offset3d {
    /// Width offset.
    pub x: ImageOffset,

    /// Height offset.
    pub y: ImageOffset,

    /// Depth offset.
    pub z: ImageOffset,
}

impl Offset3d {
    pub const ZERO: Self = Offset3d { x: 0, y: 0, z: 0 };

    pub fn from_extent(extent: Extent3d) -> Result<Self, TryFromIntError> {
        Ok(Offset3d {
            x: extent.width.try_into()?,
            y: extent.height.try_into()?,
            z: extent.depth.try_into()?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect2d {
    pub offset: Offset2d,
    pub extent: Extent2d,
}

impl From<Extent2d> for Rect2d {
    fn from(extent: Extent2d) -> Self {
        Rect2d {
            offset: Offset2d::ZERO,
            extent,
        }
    }
}

/// Error that may occur when allocation fails because of either
/// host or device memory is exhausted.
///
/// It can be matched to see which.
#[derive(Copy, Clone, Debug, thiserror::Error)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[error("Out of device memory")]
pub struct OutOfMemory;

fn merge_ordering(left: Ordering, right: Ordering) -> Option<Ordering> {
    match (left, right) {
        (Ordering::Equal, right) => Some(right),
        (left, Ordering::Equal) => Some(left),
        (left, right) if left == right => Some(left),
        _ => None,
    }
}

/// Device address is `u64` value pointing into device resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DeviceAddress(pub std::num::NonZeroU64);

impl DeviceAddress {
    pub fn offset(&mut self, offset: u64) -> DeviceAddress {
        let value = self.0.get().checked_add(offset).unwrap();

        DeviceAddress(unsafe { std::num::NonZeroU64::new_unchecked(value) })
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum IndexType {
    U16,
    U32,
}

impl IndexType {
    pub fn size(&self) -> u8 {
        match self {
            IndexType::U16 => 2,
            IndexType::U32 => 4,
        }
    }
}

#[doc(hidden)]
pub trait OrdArith<T>: Copy {
    fn cmp(self, rhs: T) -> Ordering;
}

impl<T> OrdArith<T> for T
where
    T: Ord + Copy,
{
    fn cmp(self, rhs: T) -> Ordering {
        <T as Ord>::cmp(&self, &rhs)
    }
}

impl OrdArith<u32> for usize {
    fn cmp(self, rhs: u32) -> Ordering {
        match u32::try_from(self) {
            Ok(lhs) => Ord::cmp(&lhs, &rhs),
            Err(_) => Ordering::Greater,
        }
    }
}

impl OrdArith<u64> for usize {
    fn cmp(self, rhs: u64) -> Ordering {
        match u64::try_from(self) {
            Ok(lhs) => Ord::cmp(&lhs, &rhs),
            Err(_) => Ordering::Greater,
        }
    }
}

impl OrdArith<u128> for usize {
    fn cmp(self, rhs: u128) -> Ordering {
        match u128::try_from(self) {
            Ok(lhs) => Ord::cmp(&lhs, &rhs),
            Err(_) => Ordering::Greater,
        }
    }
}

impl OrdArith<usize> for u32 {
    fn cmp(self, rhs: usize) -> Ordering {
        match u32::try_from(rhs) {
            Ok(rhs) => Ord::cmp(&self, &rhs),
            Err(_) => Ordering::Less,
        }
    }
}

impl OrdArith<usize> for u64 {
    fn cmp(self, rhs: usize) -> Ordering {
        match u64::try_from(rhs) {
            Ok(rhs) => Ord::cmp(&self, &rhs),
            Err(_) => Ordering::Less,
        }
    }
}

impl OrdArith<usize> for u128 {
    fn cmp(self, rhs: usize) -> Ordering {
        match u128::try_from(rhs) {
            Ok(rhs) => Ord::cmp(&self, &rhs),
            Err(_) => Ordering::Less,
        }
    }
}

impl OrdArith<u32> for u64 {
    fn cmp(self, rhs: u32) -> Ordering {
        Ord::cmp(&self, &u64::from(rhs))
    }
}

impl OrdArith<u32> for u128 {
    fn cmp(self, rhs: u32) -> Ordering {
        Ord::cmp(&self, &u128::from(rhs))
    }
}

impl OrdArith<u64> for u128 {
    fn cmp(self, rhs: u64) -> Ordering {
        Ord::cmp(&self, &u128::from(rhs))
    }
}

#[doc(hidden)]
pub fn arith_cmp<T>(lhs: impl OrdArith<T>, rhs: T) -> Ordering {
    lhs.cmp(rhs)
}

#[doc(hidden)]
pub fn arith_eq<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) == Ordering::Equal
}

#[doc(hidden)]
pub fn arith_ne<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) != Ordering::Equal
}

#[doc(hidden)]
pub fn arith_lt<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) == Ordering::Less
}

#[doc(hidden)]
pub fn arith_gt<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) == Ordering::Greater
}

#[doc(hidden)]
pub fn arith_le<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) != Ordering::Greater
}

#[doc(hidden)]
pub fn arith_ge<T>(lhs: impl OrdArith<T>, rhs: T) -> bool {
    lhs.cmp(rhs) != Ordering::Less
}

/// Handles host OOM the same way global allocator does.
/// This function should be called on host OOM error returned from Vulkan API.
pub fn out_of_host_memory() -> ! {
    use std::alloc::{handle_alloc_error, Layout};

    handle_alloc_error(unsafe { Layout::from_size_align_unchecked(1, 1) })
}

/// Handles host OOM the same way global allocator does.
/// This function should be called on host OOM error returned from Vulkan API.
pub fn host_memory_space_overlow() -> ! {
    panic!("Memory address space overlow")
}

fn assert_object<T: Debug + Send + Sync + 'static>() {}
fn assert_error<T: Error + Send + Sync + 'static>() {}

pub fn align_up(align_mask: u64, value: u64) -> Option<u64> {
    Some(value.checked_add(align_mask)? & !align_mask)
}

pub fn align_down(align_mask: u64, value: u64) -> u64 {
    value & !align_mask
}

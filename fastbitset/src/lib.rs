#![no_std]

#[cfg(feature = "boxed")]
extern crate alloc;

#[cfg(feature = "boxed")]
mod boxed;

#[cfg(feature = "boxed")]
pub use boxed::*;


#[cfg(feature = "bump")]
mod bump;

#[cfg(feature = "bump")]
pub use bump::*;

// macro_rules! define_handle {
//     (
//         $(#[$meta:meta])*
//         pub struct $resource:ident : $inner:ident {
//             pub info: $info:ty,
//             pub owner: $owner:ty,
//             handle: $handle:ty,
//             $($fname:ident: $fty:ty,)*
//         }
//     ) => {
//         #[doc(hiddent)]
//         pub(crate) struct $inner {
//             pub info: $info,
//             pub owner: $owner,
//             pub handle: $handle,
//             $(pub $fname: $fty,)*
//         }

//         impl ::std::fmt::Debug for $inner {
//             fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
//                 if fmt.alternate() {
//                     fmt.debug_struct(stringify!($resource))
//                         .field("handle", &self.handle)
//                         .field("info", &self.info)
//                         .field("owner", &self.owner)
//                         $(
//                             .field(stringify!($fname), &self.$fname)
//                         )*
//                         .finish()
//                 } else {
//                     write!(fmt, "{}({:p})", stringify!($resource), self.handle)
//                 }
//             }
//         }

//         #[derive(Clone)]
//         #[repr(transparent)]
//         $(#[$meta])*
//         pub struct $resource {
//              inner: std::sync::Arc<$inner>,
//         }

//         impl ::std::fmt::Debug for $resource {
//             fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
//                 self.inner.fmt(fmt)
//             }
//         }

//         impl std::cmp::PartialEq for $resource {
//             fn eq(&self, other: &Self) -> bool {
//                 std::ptr::eq(&*self.inner, &*other.inner)
//             }
//         }

//         impl std::cmp::Eq for $resource {}

//         impl std::hash::Hash for $resource {
//             fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//                 std::ptr::hash(&*self.inner, state)
//             }
//         }

//         impl $resource {
//             pub(crate) fn make(
//                 info: $info,
//                 owner: $owner,
//                 handle: $handle,
//                 $($fname: $fty,)*
//             ) -> Self {
//                 $resource {
//                     inner: std::sync::Arc::new($inner {
//                         info,
//                         owner,
//                         handle,
//                         $($fname,)*
//                     })
//                 }
//             }

//             pub fn info(&self) -> &$info {
//                 &self.inner.info
//             }

//             #[allow(unused)]
//             pub(crate) fn inner(&self, owner: &impl PartialEq<$owner>) -> &$inner {
//                 assert!(self.is_owned_by(owner), "Wrong owner");
//                 &*self.inner
//             }

//             #[allow(unused)]
//             pub(crate) unsafe fn inner_unchecked(&self) -> &$inner {
//                 &*self.inner
//             }

//             pub fn is_owned_by(&self, owner: &impl PartialEq<$owner>) -> bool {
//                 *owner == self.inner.owner
//             }
//         }
//     };
// }

macro_rules! assert_owner {
    ($resource:expr, $owner:expr) => {{
        $resource.is_owned_by(&$owner)
    }};
}

mod access;
mod convert;
mod descriptor;
mod device;
mod encode;
mod graphics;
mod physical;
mod queue;
mod resources;
mod surface;
mod swapchain;

pub use self::{
    descriptor::*, device::*, encode::*, graphics::*, physical::*, queue::*,
    resources::*, surface::*, swapchain::*,
};

#[track_caller]
fn device_lost() -> ! {
    panic!("Device lost")
}

#[track_caller]
fn unexpected_result(result: erupt::vk1_0::Result) -> ! {
    panic!("Unexpected Vulkan result {}", result)
}

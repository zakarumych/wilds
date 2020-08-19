mod swapchain;

pub use self::swapchain::*;

use crate::{
    assert_error, format::Format, image::ImageUsage, out_of_host_memory,
    Extent2d, OutOfMemory,
};
use erupt::{extensions::khr_surface::SurfaceKHR, vk1_0};
use raw_window_handle::RawWindowHandle;
use std::{
    error::Error,
    fmt::Debug,
    ops::RangeInclusive,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },

    #[error("Surfaces are not supported")]
    NotSupported,

    #[error("Image usage {{{usage:?}}} is not supported for surface images")]
    UsageNotSupported { usage: ImageUsage },

    #[error("Surface was lost")]
    SurfaceLost,

    #[error("Format {{{format:?}}} is not supported for surface images")]
    FormatUnsupported { format: Format },

    #[error(
        "Presentation mode {{{mode:?}}} is not supported for surface images"
    )]
    PresentModeUnsupported { mode: PresentMode },

    #[error("Surface is already used")]
    AlreadyUsed,

    #[error("{source}")]
    Other {
        #[cfg(target_arch = "wasm32")]
        source: Box<dyn Error + 'static>,

        #[cfg(not(target_arch = "wasm32"))]
        source: Box<dyn Error + Send + Sync + 'static>,
    },
}

#[allow(dead_code)]
fn check_surface_error() {
    assert_error::<SurfaceError>();
}

/// Kind of raw window handles
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RawWindowHandleKind {
    IOS,
    MacOS,
    Xlib,
    Xcb,
    Wayland,
    Windows,
    Web,
    Android,
    Unknown,
}

impl RawWindowHandleKind {
    /// Returns kind of the raw window handle.
    pub fn of(window: &RawWindowHandle) -> Self {
        match window {
            #[cfg(target_os = "android")]
            RawWindowHandle::Android(_) => RawWindowHandleKind::Android,

            #[cfg(target_os = "ios")]
            RawWindowHandle::IOS(_) => RawWindowHandleKind::IOS,

            #[cfg(target_os = "macos")]
            RawWindowHandle::MacOS(_) => RawWindowHandleKind::MacOS,

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Wayland(_) => RawWindowHandleKind::Wayland,

            #[cfg(target_os = "windows")]
            RawWindowHandle::Windows(_) => RawWindowHandleKind::Windows,

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Xcb(_) => RawWindowHandleKind::Xcb,

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Xlib(handle) => RawWindowHandleKind::Xlib,
            _ => RawWindowHandleKind::Unknown,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateSurfaceError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OutOfMemory,
    },
    #[error(
        "Window handle of kind {{{window:?}}} is not suppported. {source:?}"
    )]
    UnsupportedWindow {
        window: RawWindowHandleKind,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    #[error("{source}")]
    Other {
        window: RawWindowHandleKind,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum PresentMode {
    Immediate,
    Mailbox,
    Fifo,
    FifoRelaxed,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceCapabilities {
    pub families: Vec<usize>,
    pub image_count: RangeInclusive<u32>,
    pub current_extent: Extent2d,
    pub image_extent: RangeInclusive<Extent2d>,
    pub supported_usage: ImageUsage,
    pub present_modes: Vec<PresentMode>,
    pub formats: Vec<Format>,
}

#[derive(Debug)]
pub(crate) struct Inner {
    pub handle: SurfaceKHR,
    pub used: AtomicBool,
    pub info: SurfaceInfo,
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Surface {
    inner: std::sync::Arc<Inner>,
}

impl std::cmp::PartialEq for Surface {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(&*self.inner, &*other.inner)
    }
}

impl std::cmp::Eq for Surface {}

impl std::hash::Hash for Surface {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(&*self.inner, state)
    }
}

impl Surface {
    pub(crate) fn make(
        handle: SurfaceKHR,
        used: AtomicBool,
        info: SurfaceInfo,
    ) -> Self {
        Surface {
            inner: std::sync::Arc::new(Inner { handle, used, info }),
        }
    }

    pub(crate) fn handle(&self) -> SurfaceKHR {
        self.inner.handle
    }

    pub(crate) fn mark_used(&self) -> Result<(), SurfaceError> {
        if self.inner.used.fetch_or(true, Ordering::SeqCst) {
            return Err(SurfaceError::AlreadyUsed);
        } else {
            Ok(())
        }
    }

    pub fn info(&self) -> &SurfaceInfo {
        &self.inner.info
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceInfo {
    pub window: RawWindowHandle,
}

unsafe impl Send for SurfaceInfo {}
unsafe impl Sync for SurfaceInfo {}

pub(crate) fn surface_error_from_erupt(err: vk1_0::Result) -> SurfaceError {
    match err {
        vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => out_of_host_memory(),
        vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            SurfaceError::OutOfMemory {
                source: OutOfMemory,
            }
        }
        vk1_0::Result::ERROR_SURFACE_LOST_KHR => SurfaceError::SurfaceLost,
        _ => SurfaceError::Other {
            source: Box::new(err),
        },
    }
}

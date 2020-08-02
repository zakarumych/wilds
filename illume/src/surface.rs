use crate::{
    assert_error, assert_object,
    format::Format,
    image::{Image, ImageUsage},
    resource::{Handle, ResourceTrait},
    semaphore::Semaphore,
    Extent2d, OutOfMemory,
};
use raw_window_handle::RawWindowHandle;
use std::{error::Error, fmt::Debug, ops::RangeInclusive};

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

define_handle! {
    pub struct Surface(SurfaceInfo);
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceInfo {
    pub window: RawWindowHandle,
}

unsafe impl Send for SurfaceInfo {}
unsafe impl Sync for SurfaceInfo {}

#[derive(Debug)]
pub struct Swapchain {
    inner: Box<dyn SwapchainTrait>,
}

impl Swapchain {
    pub fn new(inner: Box<impl SwapchainTrait>) -> Self {
        Swapchain { inner }
    }
}

impl Swapchain {
    pub fn configure(
        &mut self,
        image_usage: ImageUsage,
        format: Format,
        mode: PresentMode,
    ) -> Result<(), SurfaceError> {
        self.inner.configure(image_usage, format, mode)
    }

    pub fn acquire_image(
        &mut self,
    ) -> Result<Option<SwapchainImage>, SurfaceError> {
        self.inner.acquire_image()
    }
}

define_handle! {
    pub struct SwapchainImage(SwapchainImageInfo);
}

#[derive(Clone, Debug)]
pub struct SwapchainImageInfo {
    /// Swapchain image.
    pub image: Image,

    /// Semaphore that should be waited upon before accessing an image.
    ///
    /// Acquisition semaphore management may be rather complex,
    /// so keep that to the implementation.
    pub wait: Semaphore,

    /// Semaphore that should be signaled after last image access.
    ///
    /// Presentation semaphore management may be rather complex,
    /// so keep that to the implementation.
    pub signal: Semaphore,
}

pub trait SwapchainTrait: Debug + Send + Sync + 'static {
    fn configure(
        &mut self,
        image_usage: ImageUsage,
        format: Format,
        mode: PresentMode,
    ) -> Result<(), SurfaceError>;

    fn acquire_image(&mut self)
        -> Result<Option<SwapchainImage>, SurfaceError>;
}

#[allow(dead_code)]
fn check() {
    assert_error::<CreateSurfaceError>();

    assert_object::<Swapchain>();
}

use {
    super::{physical::PhysicalDevice, surface::Surface},
    crate::{
        out_of_host_memory,
        physical::EnumerateDeviceError,
        surface::{CreateSurfaceError, RawWindowHandleKind, SurfaceInfo},
        OutOfMemory,
    },
    once_cell::sync::OnceCell,
    raw_window_handle::{HasRawWindowHandle, RawWindowHandle},
    smallvec::SmallVec,
    std::{
        ffi::{c_void, CStr},
        fmt::{self, Debug},
        os::raw::c_char,
        sync::atomic::AtomicBool,
    },
};

/// Root object of the erupt graphics system.
#[derive(Debug)]
pub struct Graphics {
    pub(crate) instance: wgpu::Instance,
}

static GLOBAL_GRAPHICS: OnceCell<Graphics> = OnceCell::new();

impl Graphics {
    pub fn get_or_init() -> &'static Graphics {
        GLOBAL_GRAPHICS.get_or_init(Self::new)
    }

    pub(crate) unsafe fn get_unchecked() -> &'static Graphics {
        GLOBAL_GRAPHICS.get_unchecked()
    }

    #[tracing::instrument]
    fn new() -> Self {
        tracing::trace!("Init wgpu graphisc implementation");
        Graphics {
            instance: wgpu::Instance::new(wgpu::BackendBit::all()),
        }
    }

    pub fn name(&self) -> &str {
        "WebGPU"
    }

    pub fn devices(&self) -> Result<Vec<PhysicalDevice>, EnumerateDeviceError> {
        tracing::trace!("Enumerating physical devices");

        Ok(self
            .instance
            .enumerate_adapters(wgpu::BackendBit::all())
            .map(|adapter| PhysicalDevice::new(adapter))
            .collect())
    }

    pub fn create_surface(
        &self,
        window: &impl HasRawWindowHandle,
    ) -> Result<Surface, CreateSurfaceError> {
        let surface = unsafe { self.instance.create_surface(window) };

        Ok(Surface::make(
            surface,
            AtomicBool::new(false),
            SurfaceInfo {
                window: window.raw_window_handle(),
            },
        ))
    }
}

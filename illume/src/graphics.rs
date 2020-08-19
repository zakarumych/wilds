use crate::{
    out_of_host_memory,
    physical::{EnumerateDeviceError, PhysicalDevice},
    surface::{CreateSurfaceError, RawWindowHandleKind, Surface, SurfaceInfo},
    OutOfMemory,
};
use erupt::{
    extensions::{
        ext_debug_report::{
            DebugReportCallbackCreateInfoEXT, DebugReportFlagsEXT,
            DebugReportObjectTypeEXT, ExtDebugReportInstanceLoaderExt as _,
            EXT_DEBUG_REPORT_EXTENSION_NAME,
        },
        ext_debug_utils::EXT_DEBUG_UTILS_EXTENSION_NAME,
        khr_surface::KHR_SURFACE_EXTENSION_NAME,
    },
    make_version,
    utils::loading::{DefaultCoreLoader, LibraryError},
    vk1_0::{self, Vk10CoreLoaderExt as _, Vk10InstanceLoaderExt as _},
    // vk1_1::{self, Vk11CoreLoaderExt as _, Vk11InstanceLoaderExt as _},
    InstanceLoader,
};
use once_cell::sync::OnceCell;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use smallvec::SmallVec;
use std::{
    ffi::{c_void, CStr},
    fmt::{self, Debug},
    os::raw::c_char,
    sync::atomic::AtomicBool,
};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use erupt::extensions::{
    khr_wayland_surface::{
        KhrWaylandSurfaceInstanceLoaderExt as _,
        KHR_WAYLAND_SURFACE_EXTENSION_NAME,
    },
    khr_xcb_surface::{
        KhrXcbSurfaceInstanceLoaderExt as _, KHR_XCB_SURFACE_EXTENSION_NAME,
    },
    khr_xlib_surface::{
        KhrXlibSurfaceInstanceLoaderExt as _, KHR_XLIB_SURFACE_EXTENSION_NAME,
    },
};

#[cfg(target_os = "android")]
use erupt::extensions::khr_android_surface::{
    KhrAndroidSurfaceInstanceLoaderExt as _, KHR_ANDROID_SURFACE_EXTENSION_NAME,
};

#[cfg(target_os = "windows")]
use erupt::extensions::khr_win32_surface::{
    KhrWin32SurfaceInstanceLoaderExt as _, Win32SurfaceCreateInfoKHR,
    KHR_WIN32_SURFACE_EXTENSION_NAME,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
use erupt::extensions::ext_metal_surface::{
    ExtMetalSurfaceInstanceLoaderExt as _, EXT_METAL_SURFACE_EXTENSION_NAME,
};

/// Root object of the erupt graphics system.
pub struct Graphics {
    pub(crate) instance: InstanceLoader,
    pub(crate) version: u32,
    _core: DefaultCoreLoader,
}

static GLOBAL_GRAPHICS: OnceCell<Graphics> = OnceCell::new();

impl Debug for Graphics {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("Graphics")
                .field("instance", &self.instance.handle)
                .field("version", &self.version)
                .finish()
        } else {
            Debug::fmt(&self.instance.handle, fmt)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("{source}")]
    LibraryError {
        #[from]
        source: LibraryError,
    },

    #[error("Failed to load core functions from vulkan library")]
    CoreFunctionLoadFailed,

    #[error("Failed to load advertized extension ({extension}) functions")]
    ExtensionLoadFailed { extension: &'static str },

    #[error("{source}")]
    Vulkan {
        #[from]
        source: vk1_0::Result,
    },
}

impl Graphics {
    pub fn get_or_init() -> Result<&'static Graphics, InitError> {
        GLOBAL_GRAPHICS.get_or_try_init(Self::new)
    }

    pub(crate) unsafe fn get_unchecked() -> &'static Graphics {
        GLOBAL_GRAPHICS.get_unchecked()
    }

    #[tracing::instrument]
    fn new() -> Result<Self, InitError> {
        tracing::trace!("Init erupt graphisc implementation");

        let mut core = DefaultCoreLoader::new()?;
        core.load_vk1_0().ok_or(InitError::CoreFunctionLoadFailed)?;

        // Try to load Vulkan 1.1 functions.
        let _ = core.load_vk1_1();

        let version = core.instance_version();

        let layer_properties =
            unsafe { core.enumerate_instance_layer_properties(None) }
                .result()?;

        let mut enable_layers = SmallVec::<[_; 1]>::new();

        // Pushes layer if it's avalable and returns if it was pushed.
        let mut push_layer = |name: &'static CStr| -> bool {
            if layer_properties
                .iter()
                .any(|p| unsafe { CStr::from_ptr(&p.layer_name[0]) } == name)
            {
                enable_layers.push(name.as_ptr());
                true
            } else {
                false
            }
        };

        if cfg!(debug_assertions) {
            // Enable layers in debug mode.
            if !push_layer(unsafe {
                // Safe because literal has nul-byte.
                CStr::from_bytes_with_nul_unchecked(
                    b"VK_LAYER_KHRONOS_validation\0",
                )
            }) {
                push_layer(unsafe {
                    // Safe because literal has nul-byte.
                    CStr::from_bytes_with_nul_unchecked(
                        b"VK_LAYER_LUNARG_standard_validation\0",
                    )
                });
            }

            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_LUNARG_api_dump\0") });
            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_LUNARG_device_simulation\0") });
            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_LUNARG_monitor\0") });
            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_LUNARG_screenshot\0") });

            // push_layer(unsafe {
            // CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_NV_optimus\0")
            // });

            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_NV_nomad_release_public_2020_2_0\0") });

            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_NV_GPU_Trace_release_public_2020_2_0\0") });

            // push_layer(unsafe {
            //     CStr::from_bytes_with_nul_unchecked(b"
            // VK_LAYER_LUNARG_screenshot\0") });
        }

        let extension_properties =
            unsafe { core.enumerate_instance_extension_properties(None, None) }
                .result()?;

        let mut enable_exts = SmallVec::<[_; 10]>::new();

        // Pushes extension if it's available and returns if it was pushed.
        let mut push_ext = |name| -> bool {
            let name = unsafe { CStr::from_ptr(name) };
            if extension_properties.iter().any(
                |p| unsafe { CStr::from_ptr(&p.extension_name[0]) } == name,
            ) {
                tracing::trace!("Pick extension {:?}", name);
                enable_exts.push(name.as_ptr());
                true
            } else {
                false
            }
        };

        let mut debug_utils_ext = false;
        let mut debug_report_ext = false;

        if cfg!(debug_assertions) {
            // Enable debug utils and report extensions in debug build.
            debug_utils_ext = push_ext(EXT_DEBUG_UTILS_EXTENSION_NAME);
            debug_report_ext = push_ext(EXT_DEBUG_REPORT_EXTENSION_NAME);
        }

        let mut surface = false;

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        let mut xlib_surface = false;

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        let mut xcb_surface = false;

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        let mut wayland_surface = false;

        #[cfg(target_os = "android")]
        let mut android_surface = false;

        #[cfg(target_os = "windows")]
        let mut win32_surface = false;

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        let mut metal_surface = false;

        if push_ext(KHR_SURFACE_EXTENSION_NAME) {
            surface = true;

            #[cfg(target_os = "android")]
            {
                android_surface = push_ext(KHR_ANDROID_SURFACE_EXTENSION_NAME);
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            {
                xlib_surface = push_ext(KHR_XLIB_SURFACE_EXTENSION_NAME);
                xcb_surface = push_ext(KHR_XCB_SURFACE_EXTENSION_NAME);
                wayland_surface = push_ext(KHR_WAYLAND_SURFACE_EXTENSION_NAME);
            }

            #[cfg(target_os = "windows")]
            {
                win32_surface = push_ext(KHR_WIN32_SURFACE_EXTENSION_NAME);
            }

            #[cfg(any(target_os = "ios", target_os = "macos"))]
            {
                metal_surface = push_ext(EXT_METAL_SURFACE_EXTENSION_NAME);
            }
        }

        let instance = unsafe {
            core.create_instance(
                &vk1_0::InstanceCreateInfo::default()
                    .builder()
                    .application_info(
                        &vk1_0::ApplicationInfo::default()
                            .builder()
                            .engine_name(
                                CStr::from_bytes_with_nul(b"Purr\0").unwrap(),
                            )
                            .engine_version(1)
                            .application_name(
                                CStr::from_bytes_with_nul(b"PurrApp\0")
                                    .unwrap(),
                            )
                            .application_version(1)
                            .api_version(version),
                    )
                    .enabled_layer_names(&enable_layers)
                    .enabled_extension_names(&enable_exts),
                None,
                None,
            )
        }
        .result()?;

        let mut instance = InstanceLoader::new(&core, instance)
            .ok_or(InitError::CoreFunctionLoadFailed)?;

        instance
            .load_vk1_0()
            .ok_or(InitError::CoreFunctionLoadFailed)?;

        if version >= make_version(1, 1, 0) {
            instance
                .load_vk1_1()
                .ok_or(InitError::CoreFunctionLoadFailed)?;
        }

        if debug_utils_ext {
            instance.load_ext_debug_utils().ok_or(
                InitError::ExtensionLoadFailed {
                    extension: "VK_EXT_debug_utils",
                },
            )?;
        }

        if debug_report_ext {
            instance.load_ext_debug_report().ok_or(
                InitError::ExtensionLoadFailed {
                    extension: "VK_EXT_debug_report",
                },
            )?;

            let _ = unsafe {
                instance.create_debug_report_callback_ext(
                    &DebugReportCallbackCreateInfoEXT::default()
                        .builder()
                        .flags(DebugReportFlagsEXT::all())
                        .pfn_callback(Some(debug_report_callback)),
                    None,
                    None,
                )
            }
            .result()?;
        }

        if surface {
            instance.load_khr_surface().ok_or(
                InitError::ExtensionLoadFailed {
                    extension: "VK_KHR_surface",
                },
            )?;

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            {
                if xlib_surface {
                    instance.load_khr_xlib_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_KHR_xlib_surface",
                        },
                    )?;
                }
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            {
                if xcb_surface {
                    instance.load_khr_xcb_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_KHR_xcb_surface",
                        },
                    )?;
                }
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            {
                if wayland_surface {
                    instance.load_khr_wayland_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_KHR_wayland_surface",
                        },
                    )?;
                }
            }

            #[cfg(target_os = "android")]
            {
                if android_surface {
                    instance.load_khr_android_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_KHR_android_surface",
                        },
                    )?;
                }
            }

            #[cfg(target_os = "windows")]
            {
                if win32_surface {
                    instance.load_khr_win32_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_KHR_win32_surface",
                        },
                    )?;
                }
            }

            #[cfg(any(target_os = "ios", target_os = "macos"))]
            {
                if metal_surface {
                    instance.load_ext_metal_surface().ok_or(
                        InitError::ExtensionLoadFailed {
                            extension: "VK_EXT_metal_surface",
                        },
                    )?;
                }
            }
        }

        tracing::trace!("Instance created");

        let graphics = Graphics {
            instance,
            _core: core,
            version,
        };

        Ok(graphics)
    }

    pub fn name(&self) -> &str {
        "Erupt"
    }

    pub fn devices(&self) -> Result<Vec<PhysicalDevice>, EnumerateDeviceError> {
        tracing::trace!("Enumerating physical devices");

        let devices = unsafe {
            // Using valid instance.
            self.instance.enumerate_physical_devices(None)
        }
        .result()
        .map_err(|err| match err {
            vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => out_of_host_memory(),
            vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                EnumerateDeviceError::OutOfMemory {
                    source: OutOfMemory,
                }
            }
            _ => EnumerateDeviceError::Other {
                source: Box::new(err),
            },
        })?;

        tracing::trace!("Physical devices {:?}", devices);

        Ok(devices
            .into_iter()
            .map(|physical| unsafe { PhysicalDevice::new(physical, self) })
            .collect())
    }

    pub fn create_surface(
        &self,
        window: &impl HasRawWindowHandle,
    ) -> Result<Surface, CreateSurfaceError> {
        let window = window.raw_window_handle();

        let surface = match window {
            #[cfg(target_os = "android")]
            RawWindowHandle::Android(handle) => {
                let android_surface = self
                    .android_surface_ext
                    .as_ref()
                    .ok_or_else(|| CreateSurfaceError::UnsupportedWindow {
                        window: RawWindowHandleKind::Android,
                        source: Some(Box::new(
                            RequiredExtensionIsNotAvailable {
                                extension: AndroidSurface::name(),
                            },
                        )),
                    })?;

                unimplemented!()
            }

            #[cfg(target_os = "ios")]
            RawWindowHandle::IOS(handle) => unimplemented!(),

            #[cfg(target_os = "macos")]
            RawWindowHandle::MacOS(handle) => unimplemented!(),

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Wayland(handle) => {
                let wayland_surface = self
                    .wayland_surface_ext
                    .as_ref()
                    .ok_or_else(|| CreateSurfaceError::UnsupportedWindow {
                        window: RawWindowHandleKind::Wayland,
                        source: Some(Box::new(
                            RequiredExtensionIsNotAvailable {
                                extension: WaylandSurface::name(),
                            },
                        )),
                    })?;

                unimplemented!()
            }

            #[cfg(target_os = "windows")]
            RawWindowHandle::Windows(handle) => {
                if self.instance.khr_win32_surface.is_none() {
                    return Err(CreateSurfaceError::UnsupportedWindow {
                        window: RawWindowHandleKind::Windows,
                        source: Some(Box::new(
                            RequiredExtensionIsNotAvailable {
                                extension: "VK_KHR_win32_surface",
                            },
                        )),
                    });
                }

                unsafe {
                    let mut info = Win32SurfaceCreateInfoKHR::default();
                    info.hinstance = handle.hinstance;
                    info.hwnd = handle.hwnd;
                    self.instance.create_win32_surface_khr(&info, None, None)
                }
                .result()
                .map_err(|err| match err {
                    vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => {
                        out_of_host_memory()
                    }
                    vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                        OutOfMemory.into()
                    }
                    _ => CreateSurfaceError::Other {
                        window: RawWindowHandleKind::Windows,
                        source: Box::new(err),
                    },
                })?
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Xcb(handle) => {
                if self.instance.khr_xcb_surface.is_none() {
                    return Err(CreateSurfaceError::UnsupportedWindow {
                        window: RawWindowHandleKind::Xcb,
                        source: Some(Box::new(
                            RequiredExtensionIsNotAvailable {
                                extension: KHR_XCB_SURFACE_EXTENSION_NAME,
                            },
                        )),
                    });
                }

                unimplemented!()
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            RawWindowHandle::Xlib(handle) => {
                if self.instance.khr_xlib_surface.is_none() {
                    return Err(CreateSurfaceError::UnsupportedWindow {
                        window: RawWindowHandleKind::Xlib,
                        source: Some(Box::new(
                            RequiredExtensionIsNotAvailable {
                                extension: KHR_XLIB_SURFACE_EXTENSION_NAME,
                            },
                        )),
                    });
                }

                unimplemented!()
            }
            _ => {
                debug_assert_eq!(
                    RawWindowHandleKind::of(&window),
                    RawWindowHandleKind::Unknown,
                );

                return Err(CreateSurfaceError::UnsupportedWindow {
                    window: RawWindowHandleKind::Unknown,
                    source: None,
                });
            }
        };

        Ok(Surface::make(
            surface,
            AtomicBool::new(false),
            SurfaceInfo { window },
        ))
    }
}

#[derive(Debug)]

struct RequiredExtensionIsNotAvailable {
    extension: &'static str,
}

impl fmt::Display for RequiredExtensionIsNotAvailable {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            "Required extension '{}' is not available",
            self.extension
        )
    }
}

impl std::error::Error for RequiredExtensionIsNotAvailable {}

unsafe extern "system" fn debug_report_callback(
    flags: DebugReportFlagsEXT,
    object_type: DebugReportObjectTypeEXT,
    _object: u64,
    _location: usize,
    _message_code: i32,
    p_layer_prefix: *const c_char,
    p_message: *const c_char,
    _p_user_data: *mut c_void,
) -> vk1_0::Bool32 {
    let layer_prefix = CStr::from_ptr(p_layer_prefix);

    let message = CStr::from_ptr(p_message);

    if flags.contains(DebugReportFlagsEXT::ERROR_EXT) {
        tracing::error!(
            "{:?}: {:?} | {:?}",
            layer_prefix,
            object_type,
            message
        );
    } else if flags.contains(DebugReportFlagsEXT::PERFORMANCE_WARNING_EXT) {
        tracing::warn!("{:?}: {:?} | {:?}", layer_prefix, object_type, message);
    } else if flags.contains(DebugReportFlagsEXT::WARNING_EXT) {
        tracing::warn!("{:?}: {:?} | {:?}", layer_prefix, object_type, message);
    } else if flags.contains(DebugReportFlagsEXT::INFORMATION_EXT) {
        tracing::info!("{:?}: {:?} | {:?}", layer_prefix, object_type, message);
    } else if flags.contains(DebugReportFlagsEXT::DEBUG_EXT) {
        tracing::debug!(
            "{:?}: {:?} | {:?}",
            layer_prefix,
            object_type,
            message
        );
    } else {
        tracing::trace!(
            "{:?}: {:?} | {:?}",
            layer_prefix,
            object_type,
            message
        );
    }

    0
}

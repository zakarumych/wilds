use super::{surface_error_from_erupt, PresentMode, Surface, SurfaceError};
use crate::{
    convert::{from_erupt, FromErupt as _, ToErupt as _},
    device::{Device, WeakDevice},
    format::Format,
    image::{Image, ImageInfo, ImageUsage, Samples},
    memory::MemoryUsageFlags,
    out_of_host_memory,
    semaphore::Semaphore,
    Extent2d, OutOfMemory,
};
use erupt::{
    extensions::{
        khr_surface::{self as vks, KhrSurfaceInstanceLoaderExt as _},
        khr_swapchain::SwapchainKHR,
        khr_swapchain::{self as vksw, KhrSwapchainDeviceLoaderExt as _},
    },
    vk1_0,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Weak,
};

define_handle! {
    pub struct SwapchainImage {
        pub info: SwapchainImageInfo,
        handle: SwapchainKHR,
        supported_families: Arc<Vec<bool>>,
        counter: Weak<AtomicUsize>,
    }
}

impl Drop for SwapchainImage {
    fn drop(&mut self) {
        if let Some(counter) = self.inner.counter.upgrade() {
            counter.fetch_sub(1, Ordering::Release);
        }
    }
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

#[derive(Debug)]
struct SwapchainImageAndSemaphores {
    image: Image,
    acquire: [Semaphore; 3],
    acquire_index: usize,
    release: [Semaphore; 3],
    release_index: usize,
}

#[derive(Debug)]
struct SwapchainInner {
    handle: vksw::SwapchainKHR,
    index: usize,
    images: Vec<SwapchainImageAndSemaphores>,
    counter: Arc<AtomicUsize>,
    format: Format,
    extent: Extent2d,
    usage: ImageUsage,
}

#[derive(Debug)]
pub struct Swapchain {
    inner: Option<SwapchainInner>,
    retired: Vec<SwapchainInner>,
    retired_offset: u64,
    free_semaphore: Semaphore,
    device: WeakDevice,
    surface: Surface,
    supported_families: Arc<Vec<bool>>,
}

impl Swapchain {
    pub(crate) fn new(
        surface: &Surface,
        device: &Device,
    ) -> Result<Self, SurfaceError> {
        let handle = surface.handle();

        assert!(
            device.graphics().instance.khr_surface.is_some(),
            "Should be enabled given that there is a Surface"
        );

        let instance = &device.graphics().instance;

        let supported_families = (0..device.properties().family.len() as u32)
            .map(|family| unsafe {
                instance
                    .get_physical_device_surface_support_khr(
                        device.physical(),
                        family,
                        handle,
                        None,
                    )
                    .result()
                    .map_err(|err| match err {
                        vk1_0::Result::ERROR_OUT_OF_HOST_MEMORY => {
                            out_of_host_memory()
                        }
                        vk1_0::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                            SurfaceError::OutOfMemory {
                                source: OutOfMemory,
                            }
                        }
                        vk1_0::Result::ERROR_SURFACE_LOST_KHR => {
                            SurfaceError::SurfaceLost
                        }
                        _ => unreachable!(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        surface.mark_used()?;

        Ok(Swapchain {
            surface: surface.clone(),
            free_semaphore: device
                .clone()
                .create_semaphore()
                .map_err(|err| SurfaceError::OutOfMemory { source: err })?,
            inner: None,
            retired: Vec::new(),
            retired_offset: 0,
            device: device.downgrade(),
            supported_families: Arc::new(supported_families),
        })
    }
}

impl Swapchain {
    pub fn configure(
        &mut self,
        usage: ImageUsage,
        format: Format,
        mode: PresentMode,
    ) -> Result<(), SurfaceError> {
        let device = self
            .device
            .upgrade()
            .ok_or_else(|| SurfaceError::SurfaceLost)?;

        let surface = self.surface.handle();

        assert!(
            device.graphics().instance.khr_surface.is_some(),
            "Should be enabled given that there is a Swapchain"
        );
        assert!(
            device.logical().khr_swapchain.is_some(),
            "Should be enabled given that there is a Swapchain"
        );
        let instance = &device.graphics().instance;
        let logical = &device.logical();

        let caps = unsafe {
            instance.get_physical_device_surface_capabilities_khr(
                device.physical(),
                surface,
                None,
            )
        }
        .result()
        .map_err(|err| match err {
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
        })?;

        if !ImageUsage::from_erupt(caps.supported_usage_flags).contains(usage) {
            return Err(SurfaceError::UsageNotSupported { usage });
        }

        let formats = unsafe {
            instance.get_physical_device_surface_formats_khr(
                device.physical(),
                surface,
                None,
            )
        }
        .result()
        .map_err(surface_error_from_erupt)?;

        let erupt_fromat = format.to_erupt();

        let sf = formats
            .iter()
            .find(|sf| sf.format == erupt_fromat)
            .ok_or_else(|| SurfaceError::FormatUnsupported { format })?;

        let composite_alpha = {
            let raw = caps.supported_composite_alpha.bits();

            if raw == 0 {
                tracing::warn!("Vulkan implementation must support at least one composite alpha mode, but this one reports none. Picking OPAQUE and hope for the best");
                vks::CompositeAlphaFlagsKHR::OPAQUE_KHR
            } else {
                // Use lowest bit flag
                vks::CompositeAlphaFlagsKHR::from_bits_truncate(
                    1 << raw.trailing_zeros(),
                )
            }
        };

        let modes = unsafe {
            instance.get_physical_device_surface_present_modes_khr(
                device.physical(),
                surface,
                None,
            )
        }
        .result()
        .map_err(surface_error_from_erupt)?;

        let erupt_mode = mode.to_erupt();

        if modes.iter().all(|&sm| sm != erupt_mode) {
            return Err(SurfaceError::PresentModeUnsupported { mode });
        }

        let old_swapchain = if let Some(inner) = self.inner.take() {
            let handle = inner.handle;

            self.retired.push(inner);

            handle
        } else {
            vksw::SwapchainKHR::null()
        };

        let handle = unsafe {
            logical.create_swapchain_khr(
                &vksw::SwapchainCreateInfoKHR::default()
                    .builder()
                    .surface(surface)
                    .min_image_count(
                        3.min(caps.max_image_count).max(caps.min_image_count),
                    )
                    .image_format(sf.format)
                    .image_color_space(sf.color_space)
                    .image_extent(caps.current_extent)
                    .image_array_layers(1)
                    .image_usage(usage.to_erupt())
                    .image_sharing_mode(vk1_0::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(vks::CompositeAlphaFlagBitsKHR(
                        composite_alpha.bits(),
                    ))
                    .present_mode(erupt_mode)
                    .old_swapchain(old_swapchain),
                None,
                None,
            )
        }
        .result()
        .map_err(surface_error_from_erupt)?;

        let images = unsafe {
            logical
                .get_swapchain_images_khr(handle, None)
                .result()
                .map_err(|err| {
                    logical.destroy_swapchain_khr(handle, None);
                    surface_error_from_erupt(err)
                })
        }?;

        let semaphores = (0..images.len())
            .map(|_| {
                Ok((
                    [
                        device.clone().create_semaphore()?,
                        device.clone().create_semaphore()?,
                        device.clone().create_semaphore()?,
                    ],
                    [
                        device.clone().create_semaphore()?,
                        device.clone().create_semaphore()?,
                        device.clone().create_semaphore()?,
                    ],
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| unsafe {
                logical.destroy_swapchain_khr(handle, None);

                SurfaceError::OutOfMemory { source: err }
            })?;

        let index = device.swapchains().lock().insert(handle);

        self.inner = Some(SwapchainInner {
            handle,
            index,
            images: images
                .into_iter()
                .zip(semaphores)
                .map(|(i, (a, r))| SwapchainImageAndSemaphores {
                    image: Image::make(
                        ImageInfo {
                            extent: Extent2d::from_erupt(caps.current_extent)
                                .into(),
                            format,
                            levels: 1,
                            layers: 1,
                            samples: Samples::Samples1,
                            usage,
                            memory: MemoryUsageFlags::empty(),
                        },
                        i,
                        None,
                        self.device.clone(),
                        !0,
                    ),
                    acquire: a,
                    acquire_index: 0,
                    release: r,
                    release_index: 0,
                })
                .collect(),
            counter: Arc::new(AtomicUsize::new(0)),
            extent: from_erupt(caps.current_extent),
            format,
            usage,
        });

        Ok(())
    }

    pub fn acquire_image(
        &mut self,
    ) -> Result<Option<SwapchainImage>, SurfaceError> {
        let device = self
            .device
            .upgrade()
            .ok_or_else(|| SurfaceError::SurfaceLost)?;

        assert!(
            device.logical().khr_swapchain.is_some(),
            "Should be enabled given that there is a Swapchain"
        );

        if let Some(inner) = self.inner.as_mut() {
            if inner.counter.load(Ordering::Acquire) >= inner.images.len() {
                tracing::error!("Acquire would block");
                return Ok(None);
            }

            // FIXME: Use fences to know that acqure semaphore is unused.
            let wait = self.free_semaphore.clone();

            let index = unsafe {
                device.logical().acquire_next_image_khr(
                    inner.handle,
                    !0, /* wait indefinitely. This is OK as we never try to
                         * acquire more images than there is in swaphain. */
                    unsafe { wait.handle(&device) },
                    vk1_0::Fence::null(),
                    None,
                )
            }
            .result()
            .map_err(surface_error_from_erupt)?;

            let image_and_semaphores = &mut inner.images[index as usize];

            inner.counter.fetch_add(1, Ordering::Acquire);

            std::mem::swap(
                &mut image_and_semaphores.acquire
                    [image_and_semaphores.acquire_index % 3],
                &mut self.free_semaphore,
            );

            image_and_semaphores.acquire_index += 1;

            let signal = image_and_semaphores.release
                [image_and_semaphores.release_index % 3]
                .clone();

            image_and_semaphores.release_index += 1;

            Ok(Some(SwapchainImage::make(
                SwapchainImageInfo {
                    image: image_and_semaphores.image.clone(),
                    wait,
                    signal,
                },
                inner.handle,
                self.supported_families.clone(),
                Arc::downgrade(&inner.counter),
                self.device.clone(),
                index as usize,
            )))
        } else {
            Ok(None)
        }
    }
}

impl Swapchain {
    // /// Destroys retired swapchains that are no longer used
    // ///
    // /// # Safety
    // ///
    // /// `swapchain_ext` and `logical` should belong to `self.device`.
    // /// FIXME: Wait for commands to finish too.
    // usnafe fn cleanup(&mut self, swapchain_ext: &SwapchainExt, logical:
    // &LogicalDevice) {     let to_free = self
    //         .retired
    //         .iter()
    //         .take_while(|inner| inner.acquired == 0)
    //         .count();
    //     self.retired.drain(0..to_free).for_each(|inner| {
    //         inner
    //             .images
    //             .into_iter()
    //             .for_each(|(_, s)| logical.destroy_semaphore(s, None));
    //         swapchain_ext.destroy_swapchain(inner.handle, None);
    //     });
    //     self.retired_offset += to_free as u64;
    // }
}
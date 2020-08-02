use crate::{
    assert_error, assert_object,
    device::{CreateDeviceError, CreateDeviceImplError, Device},
    queue::{Family, FamilyInfo, QueuesQuery},
    surface::{Surface, SurfaceCapabilities, SurfaceError},
    OutOfMemory as OOM,
};
use std::{error::Error, fmt::Debug};

/// Error occured during device enumeration.
#[derive(Debug, thiserror::Error)]
pub enum EnumerateDeviceError {
    #[error("{source}")]
    OutOfMemory {
        #[from]
        source: OOM,
    },

    /// Implementation specific error.
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error + Send + Sync>,
    },
}

/// Contains descriptive information about device.
#[derive(Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceInfo {
    /// Name of the device.
    pub name: String,

    /// Kind of the device.
    pub kind: Option<DeviceKind>,

    /// Features supported by device.
    pub features: Vec<Feature>,

    /// Information about queue families that device has.
    pub families: Vec<FamilyInfo>,
}

/// Kind of the device.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum DeviceKind {
    /// Device is sowtware emulated.
    Software,

    /// Device is integrate piece of hardware (typically into CPU)
    Integrated,

    /// Device is discrete piece of hardware.
    Discrete,
}

/// Features that optionally can be supported by devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub enum Feature {
    SurfacePresentation,
    BufferDeviceAddress,
    RayTracing,
    ScalarBlockLayout,
    RuntimeDescriptorArray,
    DescriptorBindingUniformBufferUpdateAfterBind,
    DescriptorBindingSampledImageUpdateAfterBind,
    DescriptorBindingStorageImageUpdateAfterBind,
    DescriptorBindingStorageBufferUpdateAfterBind,
    DescriptorBindingUniformTexelBufferUpdateAfterBind,
    DescriptorBindingStorageTexelBufferUpdateAfterBind,
    DescriptorBindingUpdateUnusedWhilePending,
    DescriptorBindingPartiallyBound,
}

/// Opaque value representing a device (software emulated of hardware).
/// Can be used to fetch information about device,
/// its support of the surface and create graphics device.
#[derive(Debug)]
#[repr(transparent)]
pub struct PhysicalDevice {
    inner: Box<dyn PhysicalDeviceTrait>,
}

impl PhysicalDevice {
    pub fn new(inner: Box<impl PhysicalDeviceTrait>) -> Self {
        PhysicalDevice { inner }
    }
}

impl PhysicalDevice {
    /// Returns information about this device.
    pub fn info(&self) -> DeviceInfo {
        self.inner.info()
    }

    /// Returns surface capabilities.
    /// Returns `Ok(None)` if this device does not support surface.
    pub fn surface_capabilities(
        &self,
        surface: &Surface,
    ) -> Result<Option<SurfaceCapabilities>, SurfaceError> {
        self.inner.surface_capabilities(surface)
    }

    /// Create graphics API device.
    ///
    /// `features` - device will enable specifeid features.
    ///     Only features listed in `DeviceInfo` returned from `self.info()` can
    /// be specified here.     Otherwise device creation will fail.
    ///
    /// `queues` - specifies `QueuesQuery` object which will query device and
    /// initialize command queues.  
    ///  Returns initialized device and queues.
    /// Type in which queues are returned depends on type of queues query,
    /// it may be single queue, an array of queues, struct, anything.
    ///
    /// Note. `QueuesQuery` may be implemented by user, this trait is not
    /// sealed.
    pub fn create_device<Q>(
        self,
        features: &[Feature],
        queues: Q,
    ) -> Result<(Device, Q::Queues), CreateDeviceError<Q::Error>>
    where
        Q: QueuesQuery,
    {
        let (query, collector) = queues
            .query(&self.inner.info().families)
            .map_err(|source| CreateDeviceError::CannotFindRequeredQueues {
                source,
            })?;

        let (device, families) =
            self.inner.create_device(features, query.as_ref())?;

        Ok((device, Q::collect(collector, families)))
    }
}

pub trait PhysicalDeviceTrait: Debug + Send + Sync + 'static {
    fn info(&self) -> DeviceInfo;

    fn surface_capabilities(
        &self,
        surface: &Surface,
    ) -> Result<Option<SurfaceCapabilities>, SurfaceError>;

    fn create_device(
        self: Box<Self>,
        features: &[Feature],
        families: &[(usize, usize)],
    ) -> Result<(Device, Vec<Family>), CreateDeviceImplError>;
}

#[allow(dead_code)]

fn check() {
    assert_error::<EnumerateDeviceError>();
    assert_object::<PhysicalDevice>();
}

use {
    super::graphics::Graphics,
    crate::{
        device::Device,
        physical::{DeviceInfo, DeviceKind, Feature},
        queue::{Family, FamilyInfo, QueueCapabilityFlags, QueuesQuery},
        CreateDeviceError,
    },
};

/// Opaque value representing a device (software emulated of hardware).
/// Can be used to fetch information about device,
/// its support of the surface and create graphics device.
#[derive(Debug)]
pub struct PhysicalDevice {
    adapter: wgpu::Adapter,
}

impl PhysicalDevice {
    pub(crate) fn new(adapter: wgpu::Adapter) -> Self {
        PhysicalDevice { adapter }
    }

    pub(crate) fn graphics(&self) -> &'static Graphics {
        unsafe {
            // PhysicalDevice can be created only via Graphics instance.
            Graphics::get_unchecked()
        }
    }

    /// Returns information about this device.
    pub fn info(&self) -> DeviceInfo {
        let info = self.adapter.get_info();
        DeviceInfo {
            kind: match info.device_type {
                wgpu::DeviceType::IntegratedGpu => Some(DeviceKind::Integrated),
                wgpu::DeviceType::DiscreteGpu => Some(DeviceKind::Discrete),
                wgpu::DeviceType::Cpu => Some(DeviceKind::Software),
                wgpu::DeviceType::Other | wgpu::DeviceType::VirtualGpu | _ => {
                    None
                }
            },
            name: info.name,
            features: vec![],
            families: vec![FamilyInfo {
                count: 1,
                capabilities: QueueCapabilityFlags::all(),
            }],
        }
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
    pub async fn create_device<Q>(
        self,
        features: &[Feature],
        queues: Q,
    ) -> Result<(Device, Q::Queues), CreateDeviceError<Q::Error>>
    where
        Q: QueuesQuery,
    {
        assert!(features.is_empty(), "WebGPU doesn't support any features");

        let (query, collector) =
            queues.query(&self.info().families).map_err(|source| {
                CreateDeviceError::CannotFindRequeredQueues { source }
            })?;

        let families = query.as_ref();
        assert_eq!(families, &[(0, 1)]);

        tracing::trace!("Creating device");

        let (device, queue) = self
            .adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: self.adapter.limits(),
                    shader_validation: cfg!(debug_assertions),
                },
                None,
            )
            .await?;

        Ok((
            Device::new(device, self),
            Q::collect(
                collector,
                vec![Family {
                    capabilities: QueueCapabilityFlags::all(),
                    queues: vec![queue],
                }],
            ),
        ))
    }
}

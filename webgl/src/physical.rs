use crate::{device::WebGlDevice, handle::*};
use illume::{
    device::{CreateDeviceImplError, Device},
    format::Format,
    image::ImageUsage,
    physical::{DeviceInfo, Feature, PhysicalDeviceTrait},
    queue::{Family, FamilyInfo, Queue, QueueCapabilityFlags, QueueId},
    surface::{PresentMode, Surface, SurfaceCapabilities, SurfaceError},
    Extent2d,
};
use std::{convert::TryInto, sync::Arc};

impl PhysicalDeviceTrait for WebGlDevice {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            name: "webgl".into(),
            kind: None,
            features: Vec::new(),
            families: vec![FamilyInfo {
                flags: QueueCapabilityFlags::GRAPHICS,
                count: 1,
            }],
        }
    }

    fn surface_capabilities(
        &self,
        surface: &Surface,
    ) -> Result<Option<SurfaceCapabilities>, SurfaceError> {
        #[derive(Debug, thiserror::Error)]
        #[error("Canvas returned negative extent")]

        struct CanvasNegativeExtent;

        if !surface.is_owner(self) {
            return Ok(None);
        }

        let extent = Extent2d {
            width: self.gl.drawing_buffer_width().try_into().map_err(|_| {
                SurfaceError::Other {
                    source: Box::new(CanvasNegativeExtent),
                }
            })?,
            height: self.gl.drawing_buffer_height().try_into().map_err(
                |_| SurfaceError::Other {
                    source: Box::new(CanvasNegativeExtent),
                },
            )?,
        };

        Ok(Some(SurfaceCapabilities {
            families: vec![0],
            image_count: 1..=1,
            current_extent: extent,
            image_extent: extent..=extent,
            supported_usage: ImageUsage::COLOR_ATTACHMENT,
            present_modes: vec![PresentMode::Fifo],
            formats: vec![Format::RGBA8Srgb],
        }))
    }

    fn create_device(
        self: Box<Self>,
        features: &[Feature],
        families: &[(usize, usize)],
    ) -> Result<(Device, Vec<Family>), CreateDeviceImplError> {
        assert!(features.is_empty());

        if families != &[(0, 1)] {
            Err(CreateDeviceImplError::BadFamiliesRequested)
        } else {
            Ok((
                Device::new(Arc::new((*self).clone())),
                vec![Family {
                    flags: QueueCapabilityFlags::GRAPHICS,
                    queues: vec![Queue::new(
                        self,
                        QueueId {
                            family: 0,
                            index: 0,
                        },
                    )],
                }],
            ))
        }
    }
}

use crate::{
    assert_object,
    physical::{EnumerateDeviceError, PhysicalDevice},
    surface::{CreateSurfaceError, Surface},
};
use maybe_sync::{MaybeSend, MaybeSync};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::{fmt::Debug, sync::Arc};

/// Graphics implementation wrapper.
///
/// Each value of `Graphics` contains pre-initialized implmentation of graphics
/// API.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Graphics {
    inner: Arc<dyn GraphicsTrait>,
}

impl Graphics {
    pub fn new(inner: Arc<impl GraphicsTrait>) -> Self {
        Graphics { inner }
    }
}

impl Graphics {
    /// Get name of the implementation.
    /// Returned string is informative
    /// and not guaranteed to be stable.
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// Create rendering surface attached to specified window.
    ///
    /// Note that some implementation (OpenGL, WebGL) are get initialized only
    /// at this point. Which means that device cannot be created before
    /// surface is created.
    pub fn create_surface(
        &self,
        window: &impl HasRawWindowHandle,
    ) -> Result<Surface, CreateSurfaceError> {
        self.inner
            .clone()
            .create_surface(window.raw_window_handle())
    }

    /// Enumerate devices for the implementation.
    /// In case concept of devices is absent for implementation
    /// this function will return semantically closest entity.
    pub fn devices(
        &self,
    ) -> Result<impl Iterator<Item = PhysicalDevice>, EnumerateDeviceError>
    {
        Ok(self.inner.clone().devices()?.into_iter())
    }
}

pub trait GraphicsTrait: Debug + MaybeSend + MaybeSync + 'static {
    fn name(&self) -> &str;

    fn devices(
        self: Arc<Self>,
    ) -> Result<Vec<PhysicalDevice>, EnumerateDeviceError>;

    fn create_surface(
        self: Arc<Self>,
        window: RawWindowHandle,
    ) -> Result<Surface, CreateSurfaceError>;
}

#[allow(dead_code)]
fn check() {
    assert_object::<Graphics>();
}

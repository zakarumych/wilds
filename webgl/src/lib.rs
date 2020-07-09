// mod convert;
// mod device;
// mod handle;
// mod image;
// mod physical;
// mod queue;
// mod swapchain;

// use self::{
//     device::WebGlDevice,
//     handle::{WebGlResource, WebGlSurface},
// };
// use illume::{
//     CreateDeviceImplError, CreateSurfaceError, Device, EnumerateDeviceError,
//     FamilyInfo, Graphics, GraphicsTrait, PhysicalDevice, PhysicalDeviceTrait,
//     QueueCapabilityFlags, RawWindowHandleKind, Surface, SurfaceInfo,
// };
// use raw_window_handle::{web::WebHandle, RawWindowHandle};
// use std::{
//     cell::{Cell, RefCell},
//     error::Error,
//     sync::Arc,
// };
// use wasm_bindgen::{prelude::*, JsCast};

// #[derive(Debug, thiserror::Error)]
// #[error("JS error occurred: {0:?}")]

// struct JsError(JsValue);

// impl From<JsError> for CreateDeviceImplError {
//     fn from(err: JsError) -> Self {
//         CreateDeviceImplError::Other {
//             source: Box::new(err),
//         }
//     }
// }

// impl From<JsError> for CreateSurfaceError {
//     fn from(err: JsError) -> Self {
//         CreateSurfaceError::Other {
//             window: RawWindowHandleKind::Web,
//             source: Box::new(err),
//         }
//     }
// }

// #[derive(Debug, thiserror::Error)]
// #[error("Type error. Value {value:?} is not of type {expected}")]

// struct TypeError {
//     value: JsValue,
//     expected: &'static str,
// }

// #[derive(Debug)]

// pub(super) struct WebGlGraphics {
//     contexts: RefCell<Vec<WebGlDevice>>,
// }

// impl WebGlGraphics {
//     pub(super) fn init() -> Option<Graphics> {
//         Some(Graphics::new(Arc::new(WebGlGraphics {
//             contexts: RefCell::new(Vec::new()),
//         })))
//     }
// }

// impl GraphicsTrait for WebGlGraphics {
//     fn name(&self) -> &str {
//         "WebGL"
//     }

//     fn devices(
//         self: Arc<Self>,
//     ) -> Result<Vec<PhysicalDevice>, EnumerateDeviceError> {
//         Ok(self
//             .contexts
//             .borrow()
//             .iter()
//             .cloned()
//             .map(|ctx| PhysicalDevice::new(Box::new(ctx)))
//             .collect())
//     }

//     fn create_surface(
//         self: Arc<Self>,
//         window: RawWindowHandle,
//     ) -> Result<Surface, CreateSurfaceError> {
//         #[derive(Debug, thiserror::Error)]
//         #[error("Failed to fetch essential object")]
//         struct EssentialObjectsNotFound;

//         #[derive(Debug, thiserror::Error)]
//         #[error("Failed to find canvas by id")]
//         struct CanvasNotFound;

//         #[derive(Debug, thiserror::Error)]
//         #[error("Failed to cast canvas node")]
//         struct WrongCanvasType;

//         #[derive(Debug, thiserror::Error)]
//         #[error("Failed to create WebGL device for canvas")]
//         struct FailedToCreateWebGlDevice;

//         match window {
//             RawWindowHandle::Web(WebHandle { id, .. }) => {
//                 let document = web_sys::window()
//                     .and_then(|w| w.document())
//                     .ok_or_else(|| CreateSurfaceError::Other {
//                         window: RawWindowHandleKind::Web,
//                         source: Box::new(EssentialObjectsNotFound),
//                     })?;

//                 let canvas = document
//                     .query_selector(&format!(
//                         "canvas[data-raw-handle=\"{}\"]",
//                         id
//                     ))
//                     .map_err(JsError)?
//                     .ok_or_else(|| CreateSurfaceError::Other {
//                         window: RawWindowHandleKind::Web,
//                         source: Box::new(CanvasNotFound),
//                     })?;

//                 let canvas = canvas
//                     .dyn_into::<web_sys::HtmlCanvasElement>()
//                     .map_err(|_| CreateSurfaceError::Other {
//                         window: RawWindowHandleKind::Web,
//                         source: Box::new(WrongCanvasType),
//                     })?;

//                 if let Some(device) = canvas
//                     .get_context_with_context_options(
//                         "webgl2",
//                         &js_sys::JSON::parse("{ \"antialias\":false }")
//                             .expect("Valid JSON object hardcoded"),
//                     )
//                     .map_err(JsError)?
//                 {
//                     let device = device.dyn_into().map_err(|_| {
//                         CreateSurfaceError::Other {
//                             window: RawWindowHandleKind::Web,
//                             source: Box::new(FailedToCreateWebGlDevice),
//                         }
//                     })?;

//                     let device = WebGlDevice::new(device);

//                     self.contexts.borrow_mut().push(device.clone());

//                     Ok(Surface::make(
//                         WebGlSurface {
//                             owner: device,
//                             used: Cell::new(false),
//                         },
//                         SurfaceInfo { window },
//                     ))
//                 } else {
//                     Err(CreateSurfaceError::Other {
//                         window: RawWindowHandleKind::Web,
//                         source: Box::new(FailedToCreateWebGlDevice),
//                     })
//                 }
//             }
//             _ => {
//                 debug_assert_eq!(
//                     RawWindowHandleKind::of(&window),
//                     RawWindowHandleKind::Unknown
//                 );

//                 Err(CreateSurfaceError::UnsupportedWindow {
//                     window: RawWindowHandleKind::Unknown,
//                     source: None,
//                 })
//             }
//         }
//     }
// }

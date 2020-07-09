use crate::resource::{Handle, ResourceTrait};

define_handle! {
    pub struct Semaphore(SemaphoreInfo);
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct SemaphoreInfo;

use erupt::vk1_0;

define_handle! {
    pub struct Semaphore {
        pub info: SemaphoreInfo,
        handle: vk1_0::Semaphore,
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct SemaphoreInfo;

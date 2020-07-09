use crate::resource::{Handle, ResourceTrait};

define_handle! {
    /// Handle to device's fence.
    /// Fence can be used to prove that queue finished execution of certain
    /// commands. See `Queue::submit`.
    pub struct Fence(FenceInfo);
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-1", derive(serde::Serialize, serde::Deserialize))]
pub struct FenceInfo;

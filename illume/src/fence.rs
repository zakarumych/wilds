use erupt::vk1_0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FenceInfo;

define_handle! {
    pub struct Fence {
        pub info: FenceInfo,
        handle: vk1_0::Fence,
    }
}

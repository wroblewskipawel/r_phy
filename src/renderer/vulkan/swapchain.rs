use ash::extensions::khr::Swapchain;
use std::ffi::CStr;
pub(super) struct VulkanSwapchain {}

impl VulkanSwapchain {
    pub const fn required_extensions() -> &'static [&'static CStr; 1] {
        const REQUIRED_DEVICE_EXTENSIONS: &[&CStr; 1] = &[Swapchain::name()];
        REQUIRED_DEVICE_EXTENSIONS
    }
}

mod layout;
mod presets;
mod type_erased;
mod type_safe;

pub use layout::*;
pub use presets::*;
pub use type_erased::*;
pub use type_safe::*;

use ash::vk;

use super::VulkanDevice;

impl VulkanDevice {
    pub fn destroy_descriptor_pool(&self, pool: impl Into<vk::DescriptorPool>) {
        unsafe {
            self.device.destroy_descriptor_pool(pool.into(), None);
        };
    }
}

use ash::vk;

use super::VulkanDevice;
use std::error::Error;
pub struct CommandPool {
    handle: vk::CommandPool,
    pub buffers: Vec<vk::CommandBuffer>,
}

impl VulkanDevice {
    pub fn create_command_pool(
        &self,
        queue_family_index: u32,
        flags: vk::CommandPoolCreateFlags,
        size: usize,
    ) -> Result<CommandPool, Box<dyn Error>> {
        let create_info = vk::CommandPoolCreateInfo {
            queue_family_index,
            flags,
            ..Default::default()
        };
        let handle = unsafe { self.device.create_command_pool(&create_info, None)? };
        let allocate_info = vk::CommandBufferAllocateInfo {
            command_pool: handle,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: size as u32,
            ..Default::default()
        };
        let buffers = unsafe { self.device.allocate_command_buffers(&allocate_info)? };
        Ok(CommandPool { handle, buffers })
    }

    pub fn destory_command_pool(&self, command_pool: &mut CommandPool) {
        unsafe {
            self.device.destroy_command_pool(command_pool.handle, None);
        }
    }
}

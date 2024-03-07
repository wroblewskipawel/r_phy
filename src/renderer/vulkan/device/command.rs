use ash::{vk, Device};

use super::{QueueFamilies, VulkanDevice};
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

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Graphics,
    Compute,
    Transfer,
}

pub enum Command {
    Graphics(vk::CommandBuffer),
    Transfer(vk::CommandBuffer),
    Compute(vk::CommandBuffer),
}

impl From<&Command> for vk::CommandBuffer {
    fn from(value: &Command) -> Self {
        match value {
            &Command::Compute(buffer) => buffer,
            &Command::Graphics(buffer) => buffer,
            &Command::Transfer(buffer) => buffer,
        }
    }
}

pub struct CommandPools {
    transfer: vk::CommandPool,
}

impl CommandPools {
    pub(super) fn create(
        device: &Device,
        queue_families: QueueFamilies,
    ) -> Result<CommandPools, Box<dyn Error>> {
        let transfer = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_families.transfer)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )?
        };
        Ok(CommandPools { transfer })
    }

    pub fn destory(&mut self, device: &Device) {
        unsafe { device.destroy_command_pool(self.transfer, None) };
    }

    pub fn allocate_command(
        &self,
        device: &Device,
        command_type: Operation,
    ) -> Result<Command, Box<dyn Error>> {
        let command_pool = match command_type {
            Operation::Transfer => self.transfer,
            _ => unimplemented!(),
        };
        let &buffer = unsafe {
            device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_pool(command_pool)
                        .command_buffer_count(1),
                )?
                .first()
                .unwrap()
        };
        Ok(match command_type {
            Operation::Transfer => Command::Transfer(buffer),
            _ => unimplemented!(),
        })
    }
    pub fn free_command(&self, device: &Device, command: Command) {
        let (buffer, command_pool) = match command {
            Command::Transfer(buffer) => (buffer, self.transfer),
            _ => unimplemented!(),
        };
        unsafe { device.free_command_buffers(command_pool, &[buffer]) };
    }
}

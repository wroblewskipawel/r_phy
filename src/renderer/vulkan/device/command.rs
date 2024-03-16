use ash::{vk, Device};

use super::{buffer::Buffer, QueueFamilies, VulkanDevice};
use std::{error::Error, ops::Index};

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Graphics,
    Compute,
    Transfer,
}

pub struct PersistentCommandPool {
    command_pool: vk::CommandPool,
    pub buffers: Vec<vk::CommandBuffer>,
}

impl VulkanDevice {
    pub(super) fn create_persistent_command_pool(
        &self,
        operation: Operation,
        size: usize,
    ) -> Result<PersistentCommandPool, Box<dyn Error>> {
        let command_pool = unsafe {
            self.device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(self.get_queue_family(operation))
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        let allocate_info = vk::CommandBufferAllocateInfo {
            command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: size as u32,
            ..Default::default()
        };
        let buffers = unsafe { self.device.allocate_command_buffers(&allocate_info)? };
        Ok(PersistentCommandPool {
            command_pool,
            buffers,
        })
    }

    pub fn destory_persistent_command_pool(&self, command_pool: &mut PersistentCommandPool) {
        unsafe {
            self.device
                .destroy_command_pool(command_pool.command_pool, None)
        };
    }
}

pub struct Command {
    operation: Operation,
    buffer: vk::CommandBuffer,
    pub fence: vk::Fence,
}

pub struct NewCommand(Command);

impl<'a> From<&'a NewCommand> for &'a Command {
    fn from(value: &'a NewCommand) -> Self {
        &value.0
    }
}

impl VulkanDevice {
    pub fn begin_command(&self, command: NewCommand) -> Result<BeginCommand, Box<dyn Error>> {
        let NewCommand(command) = command;
        unsafe {
            self.device.begin_command_buffer(
                command.buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        Ok(BeginCommand(command, self))
    }
}

pub struct BeginCommand<'a>(Command, &'a VulkanDevice);

impl<'a> From<&'a BeginCommand<'a>> for &'a Command {
    fn from(value: &'a BeginCommand) -> Self {
        &value.0
    }
}

impl<'a> BeginCommand<'a> {
    pub fn finish(self) -> Result<FinishedCommand<'a>, Box<dyn Error>> {
        let BeginCommand(command, device) = self;
        unsafe {
            device.end_command_buffer(command.buffer)?;
        }
        Ok(FinishedCommand(command, device))
    }

    pub fn copy_buffer<'b, 'c>(
        self,
        src: impl Into<&'b Buffer>,
        dst: impl Into<&'c Buffer>,
        ranges: &[vk::BufferCopy],
    ) -> Self {
        let BeginCommand(command, device) = self;
        let src = src.into();
        let dst = dst.into();
        unsafe {
            device.cmd_copy_buffer(command.buffer, src.buffer, dst.buffer, ranges);
        }
        BeginCommand(command, device)
    }
}

pub struct FinishedCommand<'a>(Command, &'a VulkanDevice);

impl<'a> From<&'a FinishedCommand<'a>> for &'a Command {
    fn from(value: &'a FinishedCommand) -> Self {
        &value.0
    }
}

impl<'a> FinishedCommand<'a> {
    pub fn submit(self) -> Result<SubmitedCommand<'a>, Box<dyn Error>> {
        let FinishedCommand(command, device) = self;
        unsafe {
            device.queue_submit(
                device.device_queues[Operation::Transfer],
                &[vk::SubmitInfo {
                    command_buffer_count: 1,
                    p_command_buffers: [command.buffer].as_ptr(),
                    ..Default::default()
                }],
                command.fence,
            )?;
        }
        Ok(SubmitedCommand(command, device))
    }
}
pub struct SubmitedCommand<'a>(Command, &'a VulkanDevice);

impl<'a> From<&'a SubmitedCommand<'a>> for &'a Command {
    fn from(value: &'a SubmitedCommand) -> Self {
        &value.0
    }
}

impl<'a> SubmitedCommand<'a> {
    pub fn wait(self) -> Result<Self, Box<dyn Error>> {
        let SubmitedCommand(command, device) = self;
        unsafe {
            device.wait_for_fences(&[command.fence], true, u64::MAX)?;
        }
        Ok(Self(command, device))
    }
}

pub struct TransientCommandPools {
    transfer: vk::CommandPool,
}

impl Index<Operation> for TransientCommandPools {
    type Output = vk::CommandPool;
    fn index(&self, index: Operation) -> &Self::Output {
        match index {
            Operation::Transfer => &self.transfer,
            _ => unimplemented!(),
        }
    }
}

impl TransientCommandPools {
    pub(super) fn create(
        device: &Device,
        queue_families: QueueFamilies,
    ) -> Result<Self, Box<dyn Error>> {
        let transfer = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_families.transfer)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )?
        };
        Ok(Self { transfer })
    }

    pub fn destory(&mut self, device: &Device) {
        unsafe { device.destroy_command_pool(self.transfer, None) };
    }
}

impl VulkanDevice {
    pub fn allocate_transient_command(
        &self,
        operation: Operation,
    ) -> Result<NewCommand, Box<dyn Error>> {
        let &buffer = unsafe {
            self.device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_pool(self.command_pools[operation])
                        .command_buffer_count(1),
                )?
                .first()
                .unwrap()
        };
        let fence = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::builder(), None)?
        };
        Ok(NewCommand(Command {
            operation,
            buffer,
            fence,
        }))
    }
    pub fn free_command<'a>(&self, command: impl Into<&'a Command>) {
        let &Command {
            buffer,
            operation,
            fence,
        } = command.into();
        unsafe {
            self.device
                .free_command_buffers(self.command_pools[operation], &[buffer]);
            self.device.destroy_fence(fence, None);
        }
    }
}

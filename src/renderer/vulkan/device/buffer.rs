use ash::vk;
use std::error::Error;

use super::VulkanDevice;

pub struct Buffer {
    pub buffer: vk::Buffer,
    device_memory: vk::DeviceMemory,
}

impl VulkanDevice {
    pub fn create_buffer(
        &self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
        memory_property_flags: vk::MemoryPropertyFlags,
    ) -> Result<Buffer, Box<dyn Error>> {
        let create_info = vk::BufferCreateInfo {
            usage,
            sharing_mode,
            size: size as u64,
            queue_family_index_count: queue_families.len() as u32,
            p_queue_family_indices: queue_families.as_ptr(),
            ..Default::default()
        };
        let (buffer, device_memory) = unsafe {
            let buffer = self.device.create_buffer(&create_info, None)?;
            let requirements = self.device.get_buffer_memory_requirements(buffer);
            let memory_type_index = self
                .get_memory_type_index(requirements.memory_type_bits, memory_property_flags)
                .ok_or("Failed to pick suitable memory type index for buffer!")?;
            let alloc_info = vk::MemoryAllocateInfo {
                allocation_size: requirements.size,
                memory_type_index,
                ..Default::default()
            };
            let device_memory = self.device.allocate_memory(&alloc_info, None)?;
            self.device.bind_buffer_memory(buffer, device_memory, 0)?;
            (buffer, device_memory)
        };
        Ok(Buffer {
            buffer,
            device_memory,
        })
    }

    pub fn transfer_buffer_data(
        &self,
        buffer: &mut Buffer,
        data: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            let p_buffer_mem = self.device.map_memory(
                buffer.device_memory,
                0,
                data.len() as vk::DeviceSize,
                vk::MemoryMapFlags::empty(),
            )?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), p_buffer_mem as *mut _, data.len());
            self.device.unmap_memory(buffer.device_memory);
        };
        Ok(())
    }

    pub fn destroy_buffer(&self, buffer: &mut Buffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.device.free_memory(buffer.device_memory, None);
        }
    }
}

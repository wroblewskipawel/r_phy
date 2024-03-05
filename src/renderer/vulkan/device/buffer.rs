use ash::{vk, Device};
use std::{error::Error, ffi::c_void, ptr::copy_nonoverlapping};

use super::VulkanDevice;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Range {
    pub size: vk::DeviceSize,
    pub offset: vk::DeviceSize,
}

pub struct MappedBufferRange<'a, 'b> {
    ptr: *mut c_void,
    device: &'a Device,
    buffer: &'b mut Buffer,
}

impl<'a, 'b> MappedBufferRange<'a, 'b> {
    pub fn copy_data(&mut self, offset: usize, data: &[u8]) {
        unsafe {
            copy_nonoverlapping(
                data.as_ptr(),
                (self.ptr as *mut u8).offset(offset as isize),
                data.len(),
            )
        }
    }
}

impl<'a, 'b> Drop for MappedBufferRange<'a, 'b> {
    fn drop(&mut self) {
        unsafe {
            self.device.unmap_memory(self.buffer.device_memory);
        }
    }
}

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

    pub fn map_buffer_range<'a, 'b>(
        &'a self,
        buffer: &'b mut Buffer,
        range: Range,
    ) -> Result<MappedBufferRange<'a, 'b>, Box<dyn Error>> {
        let ptr = unsafe {
            self.device.map_memory(
                buffer.device_memory,
                range.offset,
                range.size,
                vk::MemoryMapFlags::empty(),
            )?
        };
        Ok(MappedBufferRange {
            ptr,
            buffer: buffer,
            device: &self.device,
        })
    }

    pub fn destroy_buffer(&self, buffer: &mut Buffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.device.free_memory(buffer.device_memory, None);
        }
    }
}

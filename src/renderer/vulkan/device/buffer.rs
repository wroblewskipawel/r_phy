use ash::{vk, Device};
use bytemuck::{cast_slice, Pod};
use std::{error::Error, ffi::c_void, ptr::copy_nonoverlapping};

use crate::renderer::vulkan::device::Operation;

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

pub struct StagingBuffer<'a> {
    src_size: vk::DeviceSize,
    fence: vk::Fence,
    buffer: Buffer,
    device: &'a VulkanDevice,
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        unsafe { self.device.destroy_fence(self.fence, None) };
        self.device.destroy_buffer(&mut self.buffer);
    }
}

impl<'a> StagingBuffer<'a> {
    pub fn transfer_data(
        &mut self,
        dst: &mut Buffer,
        dst_offset: vk::DeviceSize,
    ) -> Result<(), Box<dyn Error>> {
        let command = self
            .device
            .coomand_pools
            .allocate_command(self.device, Operation::Transfer)?;
        let device: &Device = self.device;
        let command_buffer: vk::CommandBuffer = (&command).into();
        unsafe {
            device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            device.cmd_copy_buffer(
                command_buffer,
                self.buffer.buffer,
                dst.buffer,
                &[vk::BufferCopy {
                    src_offset: 0,
                    dst_offset: dst_offset,
                    size: self.src_size,
                }],
            );
            device.end_command_buffer(command_buffer)?;
            device.queue_submit(
                self.device.device_queues.transfer,
                &[vk::SubmitInfo {
                    command_buffer_count: 1,
                    p_command_buffers: [command_buffer].as_ptr(),
                    ..Default::default()
                }],
                self.fence,
            )?;
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            self.device.reset_fences(&[self.fence])?;
        }
        self.device
            .coomand_pools
            .free_command(&self.device, command);
        Ok(())
    }

    pub fn load_buffer_data_from_slices<T: Pod>(
        &mut self,
        offset: usize,
        src_slices: impl Iterator<Item = &'a [T]>,
    ) -> Result<(usize, Vec<Range>), Box<dyn Error>> {
        let mut buffer = self.device.map_buffer_range(
            &mut self.buffer,
            Range {
                offset: 0,
                size: 1024 * 1024,
            },
        )?;
        let mut buffer_offset = offset;
        let mut slice_ranges = vec![];
        for slice in src_slices {
            let bytes: &[u8] = cast_slice(slice);
            buffer.copy_data(buffer_offset, bytes);
            slice_ranges.push(Range {
                offset: buffer_offset as vk::DeviceSize,
                size: bytes.len() as vk::DeviceSize,
            });
            buffer_offset += bytes.len();
        }
        self.src_size = buffer_offset as vk::DeviceSize;
        Ok((buffer_offset, slice_ranges))
    }
}

impl VulkanDevice {
    pub fn create_stagging_buffer(&self) -> Result<StagingBuffer, Box<dyn Error>> {
        let buffer = self.create_buffer(
            1024 * 1024,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            &self.get_queue_families(&[Operation::Transfer]),
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;
        let fence = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)?
        };
        Ok(StagingBuffer {
            src_size: 0,
            fence,
            buffer,
            device: &self,
        })
    }
}

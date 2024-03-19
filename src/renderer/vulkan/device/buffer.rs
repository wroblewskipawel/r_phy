use ash::{vk, Device};
use bytemuck::{cast_slice, Pod};
use std::{error::Error, ffi::c_void, ptr::copy_nonoverlapping};

use crate::renderer::vulkan::device::command::Operation;

use super::{command::SubmitSemaphoreState, VulkanDevice};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Range {
    pub size: vk::DeviceSize,
    pub offset: vk::DeviceSize,
}

pub struct Buffer {
    pub size: usize,
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
            size,
            buffer,
            device_memory,
        })
    }

    pub fn destroy_buffer(&self, buffer: &mut Buffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.device.free_memory(buffer.device_memory, None);
        }
    }
}

pub struct HostVisibleBuffer {
    buffer: Buffer,
}

impl<'a> From<&'a HostVisibleBuffer> for &'a Buffer {
    fn from(value: &'a HostVisibleBuffer) -> Self {
        &value.buffer
    }
}

impl<'a> From<&'a mut HostVisibleBuffer> for &'a mut Buffer {
    fn from(value: &'a mut HostVisibleBuffer) -> Self {
        &mut value.buffer
    }
}

impl HostVisibleBuffer {
    fn map(&mut self, device: &Device, range: Range) -> Result<HostMappedMemory, Box<dyn Error>> {
        let ptr = unsafe {
            device.map_memory(
                self.buffer.device_memory,
                range.offset,
                range.size,
                vk::MemoryMapFlags::empty(),
            )?
        };
        Ok(HostMappedMemory {
            device_memory: self.buffer.device_memory,
            ptr: Some(ptr),
        })
    }
}

pub struct HostMappedMemory {
    device_memory: vk::DeviceMemory,
    ptr: Option<*mut c_void>,
}

impl HostMappedMemory {
    fn transfer_data<T: Pod>(
        &mut self,
        dst_offset: vk::DeviceSize,
        src: &[T],
    ) -> Result<&mut Self, Box<dyn Error>> {
        let ptr = self
            .ptr
            .ok_or("Host Visible buffer isn't currently mapped!")?;
        let src_bytes: &[u8] = cast_slice(src);
        unsafe {
            copy_nonoverlapping(
                src_bytes.as_ptr(),
                (ptr as *mut u8).offset(dst_offset as isize),
                src_bytes.len(),
            )
        };
        Ok(self)
    }

    fn unmap(&mut self, device: &Device) {
        if let Some(_) = self.ptr.take() {
            unsafe { device.unmap_memory(self.device_memory) };
        }
    }
}

impl Drop for HostMappedMemory {
    fn drop(&mut self) {
        if let Some(_) = self.ptr.take() {
            panic!("HostMappedMemory wasn't unmapped before drop!");
        }
    }
}

impl VulkanDevice {
    pub fn create_host_visible_buffer(
        &self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
    ) -> Result<HostVisibleBuffer, Box<dyn Error>> {
        let buffer = self.create_buffer(
            size,
            usage,
            sharing_mode,
            queue_families,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;
        Ok(HostVisibleBuffer { buffer })
    }
}

pub struct DeviceLocalBuffer {
    pub buffer: Buffer,
}

impl<'a> From<&'a DeviceLocalBuffer> for &'a Buffer {
    fn from(value: &'a DeviceLocalBuffer) -> Self {
        &value.buffer
    }
}

impl<'a> From<&'a mut DeviceLocalBuffer> for &'a mut Buffer {
    fn from(value: &'a mut DeviceLocalBuffer) -> Self {
        &mut value.buffer
    }
}

impl VulkanDevice {
    pub fn create_device_local_buffer(
        &self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
    ) -> Result<DeviceLocalBuffer, Box<dyn Error>> {
        let buffer = self.create_buffer(
            size,
            usage,
            sharing_mode,
            queue_families,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        Ok(DeviceLocalBuffer { buffer })
    }
}

pub struct StagingBuffer<'a> {
    buffer: HostVisibleBuffer,
    offset: vk::DeviceSize,
    device: &'a VulkanDevice,
}

impl<'a> From<&'a StagingBuffer<'a>> for &'a Buffer {
    fn from(value: &'a StagingBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a> From<&'a mut StagingBuffer<'a>> for &'a mut Buffer {
    fn from(value: &'a mut StagingBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        self.device.destroy_buffer((&mut self.buffer).into())
    }
}

impl<'a> StagingBuffer<'a> {
    pub fn transfer_data<'b>(
        &self,
        dst: impl Into<&'b Buffer>,
        dst_offset: vk::DeviceSize,
    ) -> Result<(), Box<dyn Error>> {
        let command = self
            .device
            .allocate_transient_command(Operation::Transfer)?;
        let command = self.device.begin_command(command)?;
        let command = self.device.record_command(command, |command| {
            command.copy_buffer(
                &self.buffer,
                dst,
                &[vk::BufferCopy {
                    src_offset: 0,
                    dst_offset: dst_offset,
                    size: self.offset,
                }],
            )
        });
        let command = self
            .device
            .finish_command(command)?
            .submit(
                SubmitSemaphoreState {
                    semaphores: &[],
                    masks: &[],
                },
                &[],
            )?
            .wait()?;
        self.device.free_command(&command);
        Ok(())
    }

    pub fn load_buffer_data_from_slices<T: Pod>(
        &mut self,
        src_slices: &[&[T]],
        alignment: usize,
    ) -> Result<(usize, Vec<Range>), Box<dyn Error>> {
        let mut p_buffer = self.buffer.map(
            &self.device,
            Range {
                offset: 0,
                size: self.buffer.buffer.size as u64,
            },
        )?;
        let mut buffer_offset = Self::align_offset(self.offset, alignment as vk::DeviceSize);
        let mut slice_ranges = vec![];
        for slice in src_slices {
            let bytes: &[u8] = cast_slice(slice);
            p_buffer.transfer_data(buffer_offset, bytes)?;
            slice_ranges.push(Range {
                offset: buffer_offset as vk::DeviceSize,
                size: bytes.len() as vk::DeviceSize,
            });
            buffer_offset += bytes.len() as u64;
        }
        self.offset = buffer_offset as vk::DeviceSize;
        p_buffer.unmap(self.device);
        Ok((buffer_offset as usize, slice_ranges))
    }

    fn align_offset(offset: vk::DeviceSize, alignment: vk::DeviceSize) -> vk::DeviceSize {
        debug_assert_ne!(alignment, 0, "Invalid alignment value!");
        ((offset + (alignment - 1)) / alignment) * alignment
    }
}

impl VulkanDevice {
    pub fn create_stagging_buffer(&self, size: usize) -> Result<StagingBuffer, Box<dyn Error>> {
        let buffer = self.create_host_visible_buffer(
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            &self.get_queue_families(&[Operation::Transfer]),
        )?;
        Ok(StagingBuffer {
            offset: 0,
            buffer,
            device: self,
        })
    }
}

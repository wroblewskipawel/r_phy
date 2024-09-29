use std::{error::Error, marker::PhantomData};

use ash::vk;

use crate::{
    core::Nil,
    renderer::vulkan::device::{
        memory::{MemoryChunk, MemoryChunkRaw, MemoryProperties},
        resources::buffer::ByteRange,
        VulkanDevice,
    },
};

use super::{AllocReq, Allocator, AllocatorCreate, DeviceAllocError};

pub struct DefaultAllocator {}

impl AllocatorCreate for DefaultAllocator {
    type Config = Nil;

    fn create(_device: &VulkanDevice, _config: &Self::Config) -> Result<Self, Box<dyn Error>> {
        Ok(DefaultAllocator {})
    }

    fn destroy(&mut self, _device: &VulkanDevice) {}
}

impl Allocator for DefaultAllocator {
    type Allocation<M: MemoryProperties> = MemoryChunk<M>;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &VulkanDevice,
        request: AllocReq<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError> {
        let memory_type_index = request
            .get_memory_type_index(&device.physical_device.properties.memory)
            .ok_or(DeviceAllocError::UnsupportedMemoryType)?;
        let memory = unsafe {
            device.allocate_memory(
                &vk::MemoryAllocateInfo {
                    allocation_size: request.requirements.size,
                    memory_type_index,
                    ..Default::default()
                },
                None,
            )?
        };
        Ok(MemoryChunk {
            raw: MemoryChunkRaw {
                memory,
                range: ByteRange::new(request.requirements.size as usize),
            },
            _phantom: PhantomData,
        })
    }

    fn free<M: MemoryProperties>(
        &mut self,
        device: &VulkanDevice,
        allocation: &mut Self::Allocation<M>,
    ) {
        unsafe {
            device.free_memory(allocation.memory, None);
        }
        *allocation = MemoryChunk::empty();
    }
}

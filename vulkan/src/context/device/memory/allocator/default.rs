use std::{error::Error, marker::PhantomData};

use ash::vk;

use crate::context::{
    device::{
        memory::{MemoryChunk, MemoryChunkRaw, MemoryProperties},
        resources::buffer::ByteRange,
        Device,
    },
    error::AllocError,
};
use type_kit::Nil;

use super::{AllocReqTyped, Allocator, AllocatorCreate};

pub struct DefaultAllocator {}

impl AllocatorCreate for DefaultAllocator {
    type Config = Nil;

    fn create(_device: &Device, _config: &Self::Config) -> Result<Self, Box<dyn Error>> {
        Ok(DefaultAllocator {})
    }

    fn destroy(&mut self, _device: &Device) {}
}

impl Allocator for DefaultAllocator {
    type Allocation<M: MemoryProperties> = MemoryChunk<M>;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        request: AllocReqTyped<M>,
    ) -> Result<Self::Allocation<M>, AllocError> {
        let memory_type_index = request
            .get_memory_type_index(&device.physical_device.properties.memory)
            .ok_or(AllocError::UnsupportedMemoryType)?;
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

    fn free<M: MemoryProperties>(&mut self, device: &Device, allocation: &mut Self::Allocation<M>) {
        unsafe {
            device.free_memory(allocation.memory, None);
        }
        *allocation = MemoryChunk::empty();
    }
}

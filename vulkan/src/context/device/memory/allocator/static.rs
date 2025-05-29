use std::{error::Error, marker::PhantomData};

use ash::vk::{self, MemoryRequirements, PhysicalDeviceMemoryProperties};

use crate::context::{
    device::{
        memory::{MemoryChunk, MemoryChunkRaw, MemoryProperties},
        resources::buffer::ByteRange,
        Device,
    },
    error::{AllocError, AllocResult},
};

use super::{AllocReq, AllocReqTyped, Allocator, AllocatorCreate};

#[derive(Debug, Default)]
pub struct StaticAllocatorConfig {
    properties: PhysicalDeviceMemoryProperties,
    allocations: Vec<ByteRange>,
}

impl StaticAllocatorConfig {
    pub fn create(device: &Device) -> Self {
        let properties = &device.physical_device.properties.memory;
        Self {
            properties: properties.clone(),
            allocations: vec![ByteRange::empty(); properties.memory_type_count as usize],
        }
    }

    pub fn add_allocation(&mut self, req: AllocReq) {
        let MemoryRequirements {
            size, alignment, ..
        } = req.requirements();
        let memory_type_index = req.get_memory_type_index(&self.properties).unwrap() as usize;
        self.allocations[memory_type_index].extend_raw(size as usize, alignment as usize);
    }
}

pub struct StaticAllocator {
    allocations: Vec<MemoryChunkRaw>,
}

impl AllocatorCreate for StaticAllocator {
    type Config = StaticAllocatorConfig;

    fn create(device: &Device, config: &Self::Config) -> Result<Self, Box<dyn std::error::Error>> {
        let allocations = config
            .allocations
            .iter()
            .enumerate()
            .map(|(index, range)| {
                let memory = if range.len() != 0 {
                    MemoryChunkRaw {
                        memory: unsafe {
                            device.allocate_memory(
                                &vk::MemoryAllocateInfo {
                                    allocation_size: range.len() as vk::DeviceSize,
                                    memory_type_index: index as u32,
                                    ..Default::default()
                                },
                                None,
                            )?
                        },
                        range: range.clone(),
                    }
                } else {
                    MemoryChunkRaw {
                        memory: vk::DeviceMemory::null(),
                        range: ByteRange::empty(),
                    }
                };
                Result::<_, Box<dyn Error>>::Ok(memory)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StaticAllocator { allocations })
    }

    fn destroy(&mut self, device: &Device) {
        self.allocations.drain(0..).for_each(|alloc| {
            if alloc.memory != vk::DeviceMemory::null() {
                unsafe {
                    device.free_memory(alloc.memory, None);
                }
            }
        });
    }
}

impl Allocator for StaticAllocator {
    type Allocation<M: MemoryProperties> = MemoryChunk<M>;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        req: AllocReqTyped<M>,
    ) -> AllocResult<Self::Allocation<M>> {
        let MemoryRequirements {
            size, alignment, ..
        } = req.requirements;
        let memory_type_index = req
            .get_memory_type_index(&device.physical_device.properties.memory)
            .ok_or(AllocError::UnsupportedMemoryType)? as usize;
        let allocation = &mut self.allocations[memory_type_index];
        Ok(MemoryChunk {
            raw: MemoryChunkRaw {
                memory: allocation.memory,
                range: allocation
                    .range
                    .alloc_raw(size as usize, alignment as usize)
                    .ok_or(AllocError::OutOfMemory)?,
            },
            _phantom: PhantomData,
        })
    }

    fn free<M: MemoryProperties>(
        &mut self,
        _device: &Device,
        _allocation: &mut Self::Allocation<M>,
    ) {
    }
}

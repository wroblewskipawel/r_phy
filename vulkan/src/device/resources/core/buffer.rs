mod persistent;
mod range;
mod staging;
mod uniform;

pub use persistent::*;
pub use range::*;
pub use staging::*;
pub use uniform::*;

use ash::vk;

use std::{error::Error, marker::PhantomData, usize};

use crate::device::{
    memory::{AllocReq, AllocReqTyped, Allocator, MemoryProperties},
    Device,
};

use super::PartialBuilder;

#[derive(Debug, Clone, Copy)]
pub struct BufferInfo<'a> {
    pub size: usize,
    pub usage: vk::BufferUsageFlags,
    pub sharing_mode: vk::SharingMode,
    pub queue_families: &'a [u32],
}

pub struct BufferBuilder<'a, M: MemoryProperties> {
    pub info: BufferInfo<'a>,
    _phantom: PhantomData<M>,
}

impl<'a, M: MemoryProperties> BufferBuilder<'a, M> {
    pub fn new(info: BufferInfo<'a>) -> Self {
        Self {
            info,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct Buffer<M: MemoryProperties, A: Allocator> {
    size: usize,
    buffer: vk::Buffer,
    memory: A::Allocation<M>,
}

impl<M: MemoryProperties, A: Allocator> Buffer<M, A> {
    pub fn handle(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

#[derive(Debug)]
pub struct BufferPartial<M: MemoryProperties> {
    size: usize,
    req: AllocReqTyped<M>,
    buffer: vk::Buffer,
}

impl<'a, M: MemoryProperties> PartialBuilder<'a> for BufferPartial<M> {
    type Config = BufferBuilder<'a, M>;
    type Target<A: Allocator> = Buffer<M, A>;

    fn prepare(config: Self::Config, device: &Device) -> Result<Self, Box<dyn Error>> {
        let BufferBuilder {
            info:
                BufferInfo {
                    size,
                    usage,
                    sharing_mode,
                    queue_families,
                },
            ..
        } = config;
        let create_info = vk::BufferCreateInfo {
            usage,
            sharing_mode,
            size: size as u64,
            queue_family_index_count: queue_families.len() as u32,
            p_queue_family_indices: queue_families.as_ptr(),
            ..Default::default()
        };
        let buffer = unsafe { device.create_buffer(&create_info, None)? };
        let req = device.get_alloc_req(buffer);
        Ok(BufferPartial { size, req, buffer })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        [self.req.into()].into_iter()
    }

    fn finalize<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>> {
        let BufferPartial { size, buffer, req } = self;
        let memory = allocator.allocate(device, req)?;
        device.bind_memory(buffer, &memory)?;
        Ok(Buffer {
            size,
            buffer,
            memory,
        })
    }
}

impl Device {
    pub fn destroy_buffer<'a, M: MemoryProperties, A: Allocator>(
        &self,
        buffer: impl Into<&'a mut Buffer<M, A>>,
        allocator: &mut A,
    ) {
        let buffer = buffer.into();
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            allocator.free(self, &mut buffer.memory);
        }
    }
}

mod persistent;
mod range;
mod staging;
mod uniform;

pub use persistent::*;
pub use range::*;
pub use staging::*;
use type_kit::{Create, Destroy};
pub use uniform::*;

use ash::vk;

use std::{marker::PhantomData, usize};

use crate::{
    device::{
        memory::{AllocReq, AllocReqTyped, Allocator, MemoryProperties},
        Device,
    },
    error::{VkError, VkResult},
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

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self> {
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
}

impl<M: MemoryProperties, A: Allocator> Create for Buffer<M, A> {
    type Config<'a> = BufferPartial<M>;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let BufferPartial { size, buffer, req } = config;
        let memory = allocator.allocate(device, req)?;
        device.bind_memory(buffer, &memory)?;
        Ok(Buffer {
            size,
            buffer,
            memory,
        })
    }
}

impl<M: MemoryProperties, A: Allocator> Destroy for Buffer<M, A> {
    type Context<'a> = (&'a Device, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        unsafe {
            device.destroy_buffer(self.buffer, None);
        }
        allocator.free(device, &mut self.memory);
    }
}

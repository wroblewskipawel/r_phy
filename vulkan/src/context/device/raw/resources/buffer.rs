use std::{convert::Infallible, marker::PhantomData};

use ash::vk;
use type_kit::{Create, CreateResult, Destroy, DestroyResult, FromGuard, Valid};

use crate::context::{device::memory::MemoryProperties, error::ResourceError, Context};

use super::Resource;

#[derive(Debug, Clone, Copy)]
pub struct BufferRaw {
    handle: vk::Buffer,
    size: vk::DeviceSize,
}

#[derive(Debug)]
pub struct Buffer<M: MemoryProperties> {
    handle: vk::Buffer,
    size: vk::DeviceSize,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> From<Valid<Buffer<M>>> for Buffer<M> {
    fn from(guard: Valid<Buffer<M>>) -> Self {
        let inner = guard.into_inner();
        Self {
            handle: inner.handle,
            size: inner.size,
            _phantom: PhantomData,
        }
    }
}

impl<M: MemoryProperties> FromGuard for Buffer<M> {
    type Inner = BufferRaw;

    #[inline]
    fn into_inner(self) -> Self::Inner {
        BufferRaw {
            handle: self.handle,
            size: self.size,
        }
    }
}

impl<M: MemoryProperties> Resource for Buffer<M> {
    type RawType = BufferRaw;
}

impl<M: MemoryProperties> Create for Buffer<M> {
    type Config<'a> = vk::BufferCreateInfo;
    type CreateError = ResourceError;

    #[inline]
    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let buffer = Buffer {
            handle: unsafe { context.create_buffer(&config, None)? },
            size: config.size,
            _phantom: PhantomData,
        };
        Ok(buffer)
    }
}

impl Destroy for BufferRaw {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        unsafe {
            context.destroy_buffer(self.handle, None);
        }
        Ok(())
    }
}

impl<M: MemoryProperties> Destroy for Buffer<M> {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        unsafe {
            context.destroy_buffer(self.handle, None);
        }
        Ok(())
    }
}

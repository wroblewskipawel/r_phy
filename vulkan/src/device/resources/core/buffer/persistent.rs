use std::{cell::RefCell, convert::Infallible, ffi::c_void};

use type_kit::{Create, Destroy, DestroyResult};

use crate::{
    device::{
        memory::{AllocReq, Allocator, HostCoherent, Memory},
        resources::PartialBuilder,
        Device,
    },
    error::{VkError, VkResult},
};

use super::{Buffer, BufferBuilder, BufferPartial, ByteRange};

pub struct PersistentBufferPartial {
    buffer: BufferPartial<HostCoherent>,
}

pub struct PersistentBuffer<A: Allocator> {
    pub buffer: Buffer<HostCoherent, A>,
    pub ptr: Option<*mut c_void>,
}

impl<'a, A: Allocator> From<&'a PersistentBuffer<A>> for &'a Buffer<HostCoherent, A> {
    fn from(value: &'a PersistentBuffer<A>) -> Self {
        &value.buffer
    }
}

impl<'a, A: Allocator> From<&'a mut PersistentBuffer<A>> for &'a mut Buffer<HostCoherent, A> {
    fn from(value: &'a mut PersistentBuffer<A>) -> Self {
        &mut value.buffer
    }
}

impl<'a> PartialBuilder<'a> for PersistentBufferPartial {
    type Config = BufferBuilder<'a, HostCoherent>;
    type Target<A: Allocator> = PersistentBuffer<A>;

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self> {
        let buffer = BufferPartial::prepare(config, device)?;
        Ok(PersistentBufferPartial { buffer })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        self.buffer.requirements()
    }
}

impl<A: Allocator> Create for PersistentBuffer<A> {
    type Config<'a> = PersistentBufferPartial;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let mut buffer = Buffer::create(config.buffer, (device, allocator))?;
        let ptr = buffer.memory.map(
            &device,
            ByteRange {
                beg: 0,
                end: buffer.size,
            },
        )?;
        Ok(PersistentBuffer {
            buffer,
            ptr: Some(ptr),
        })
    }
}

impl<A: Allocator> Destroy for PersistentBuffer<A> {
    type Context<'a> = (&'a Device, &'a RefCell<&'a mut A>);
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        let (device, allocator) = context;
        if let Some(..) = self.ptr {
            self.buffer.memory.unmap(device);
            self.ptr = None;
        }
        self.buffer.destroy((device, allocator))?;
        Ok(())
    }
}

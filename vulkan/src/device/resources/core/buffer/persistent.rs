use std::{error::Error, ffi::c_void};

use crate::device::{
    memory::{AllocReq, Allocator, HostCoherent, Memory},
    resources::PartialBuilder,
    Device,
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

    fn prepare(config: Self::Config, device: &Device) -> Result<Self, Box<dyn Error>> {
        let buffer = BufferPartial::prepare(config, device)?;
        Ok(PersistentBufferPartial { buffer })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        self.buffer.requirements()
    }

    fn finalize<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>> {
        let mut buffer = self.buffer.finalize(device, allocator)?;
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

use std::{
    any::{type_name, TypeId},
    error::Error,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use ash::vk;
use bytemuck::AnyBitPattern;

use crate::device::{
    command::operation::Operation,
    memory::{AllocReq, Allocator, HostCoherent},
    resources::PartialBuilder,
    Device,
};

use super::{Buffer, BufferBuilder, BufferInfo, PersistentBuffer, PersistentBufferPartial};

pub struct UniformBuffer<U: AnyBitPattern, O: Operation, A: Allocator> {
    len: usize,
    buffer: PersistentBuffer<A>,
    _phantom: PhantomData<(U, O)>,
}

pub struct UniformBufferPartial<U: AnyBitPattern, O: Operation> {
    len: usize,
    buffer: PersistentBufferPartial,
    _phantom: PhantomData<(U, O)>,
}

pub struct UniformBufferBuilder<U: AnyBitPattern, O: Operation> {
    len: usize,
    _phantom: PhantomData<(U, O)>,
}

impl<U: AnyBitPattern, O: Operation> UniformBufferBuilder<U, O> {
    pub fn new(len: usize) -> Self {
        Self {
            len,
            _phantom: PhantomData,
        }
    }
}

impl<'a, U: AnyBitPattern, O: Operation> PartialBuilder<'a> for UniformBufferPartial<U, O> {
    type Config = UniformBufferBuilder<U, O>;
    type Target<A: Allocator> = UniformBuffer<U, O, A>;

    fn prepare(config: Self::Config, device: &Device) -> Result<Self, Box<dyn Error>> {
        let info = BufferInfo {
            size: size_of::<U>() * config.len,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[O::get_queue_family_index(device)],
        };
        let buffer = PersistentBufferPartial::prepare(BufferBuilder::new(info), device)?;
        Ok(UniformBufferPartial {
            len: config.len,
            buffer,
            _phantom: PhantomData,
        })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        self.buffer.requirements()
    }

    fn finalize<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>> {
        let len = self.len;
        let buffer = self.buffer.finalize(device, allocator)?;
        Ok(UniformBuffer {
            len,
            buffer,
            _phantom: PhantomData,
        })
    }
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a UniformBuffer<U, O, A>>
    for &'a Buffer<HostCoherent, A>
{
    fn from(value: &'a UniformBuffer<U, O, A>) -> Self {
        &value.buffer.buffer
    }
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a mut UniformBuffer<U, O, A>>
    for &'a mut Buffer<HostCoherent, A>
{
    fn from(value: &'a mut UniformBuffer<U, O, A>) -> Self {
        &mut value.buffer.buffer
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBuffer<U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize> for UniformBuffer<U, O, A> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> UniformBuffer<U, O, A> {
    pub fn handle(&self) -> vk::Buffer {
        self.buffer.buffer.handle()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

// TODO: Move to separate module
pub struct UniformBufferTypeErased<O: Operation, A: Allocator> {
    len: usize,
    buffer: PersistentBuffer<A>,
    type_id: TypeId,
    _phantom: PhantomData<O>,
}

impl<P: AnyBitPattern, O: Operation, A: Allocator> From<UniformBuffer<P, O, A>>
    for UniformBufferTypeErased<O, A>
{
    fn from(value: UniformBuffer<P, O, A>) -> Self {
        let UniformBuffer { len, buffer, .. } = value;
        UniformBufferTypeErased {
            len,
            buffer,
            type_id: TypeId::of::<P>(),
            _phantom: PhantomData,
        }
    }
}

pub struct UniformBufferRef<'a, P: AnyBitPattern, O: Operation, A: Allocator> {
    len: usize,
    buffer: &'a mut PersistentBuffer<A>,
    _phantom: PhantomData<(P, O)>,
}

impl<'a, P: AnyBitPattern, O: Operation, A: Allocator>
    TryFrom<&'a mut UniformBufferTypeErased<O, A>> for UniformBufferRef<'a, P, O, A>
{
    type Error = Box<dyn Error>;

    fn try_from(value: &'a mut UniformBufferTypeErased<O, A>) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<P>() {
            Ok(UniformBufferRef {
                len: value.len,
                buffer: &mut value.buffer,
                _phantom: PhantomData,
            })
        } else {
            Err(format!(
                "Invalid uniform data type {} for uniform buffer!",
                type_name::<P>()
            ))?
        }
    }
}

impl<'a, O: Operation, A: Allocator> From<&'a mut UniformBufferTypeErased<O, A>>
    for &'a mut Buffer<HostCoherent, A>
{
    fn from(value: &'a mut UniformBufferTypeErased<O, A>) -> Self {
        (&mut value.buffer).into()
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBufferRef<'_, U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize>
    for UniformBufferRef<'_, U, O, A>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}

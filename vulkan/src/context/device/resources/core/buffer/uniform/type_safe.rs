use std::{
    cell::RefCell,
    convert::Infallible,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use ash::vk;
use bytemuck::AnyBitPattern;
use type_kit::{Create, CreateResult, Destroy, DestroyResult};

use crate::context::{
    device::{
        command::operation::Operation,
        memory::{AllocReq, Allocator, HostCoherent},
        resources::{
            buffer::{
                Buffer, BufferBuilder, BufferInfo, PersistentBuffer, PersistentBufferPartial,
            },
            PartialBuilder,
        },
        Device,
    },
    error::{VkError, VkResult},
};

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

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self> {
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

impl<U: AnyBitPattern, O: Operation, A: Allocator> Create for UniformBuffer<U, O, A> {
    type Config<'a> = UniformBufferPartial<U, O>;
    type CreateError = VkError;

    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let (device, allocator) = context;
        let len = config.len;
        let buffer = PersistentBuffer::create(config.buffer, (device, allocator))?;
        Ok(UniformBuffer {
            len,
            buffer,
            _phantom: PhantomData,
        })
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Destroy for UniformBuffer<U, O, A> {
    type Context<'a> = (&'a Device, &'a RefCell<&'a mut A>);
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.buffer.destroy(context)?;
        Ok(())
    }
}

use std::{
    any::{type_name, TypeId},
    cell::RefCell,
    convert::Infallible,
    error::Error,
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

pub struct UniformBufferErasedPartial<O: Operation> {
    len: usize,
    buffer: PersistentBufferPartial,
    item_type_id: TypeId,
    _phantom: PhantomData<O>,
}

pub struct UniformBufferErasedBuilder<O: Operation> {
    len: usize,
    item_size: usize,
    item_type_id: TypeId,
    _phantom: PhantomData<O>,
}

impl<O: Operation> UniformBufferErasedBuilder<O> {
    pub fn new<U: AnyBitPattern>(len: usize) -> Self {
        Self {
            len,
            item_size: size_of::<U>(),
            item_type_id: TypeId::of::<U>(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, O: Operation> PartialBuilder<'a> for UniformBufferErasedPartial<O> {
    type Config = UniformBufferErasedBuilder<O>;
    type Target<A: Allocator> = UniformBufferTypeErased<O, A>;

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self> {
        let UniformBufferErasedBuilder {
            len,
            item_size,
            item_type_id,
            ..
        } = config;
        let info = BufferInfo {
            size: item_size * config.len,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[O::get_queue_family_index(device)],
        };
        let buffer = PersistentBufferPartial::prepare(BufferBuilder::new(info), device)?;
        Ok(UniformBufferErasedPartial {
            len,
            buffer,
            item_type_id,
            _phantom: PhantomData,
        })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        self.buffer.requirements()
    }
}

pub struct UniformBufferTypeErased<O: Operation, A: Allocator> {
    len: usize,
    buffer: PersistentBuffer<A>,
    item_type_id: TypeId,
    _phantom: PhantomData<O>,
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
        if value.item_type_id == TypeId::of::<P>() {
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

impl<O: Operation, A: Allocator> Create for UniformBufferTypeErased<O, A> {
    type Config<'a> = UniformBufferErasedPartial<O>;
    type CreateError = VkError;

    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let (device, allocator) = context;
        let UniformBufferErasedPartial {
            len,
            buffer,
            item_type_id,
            ..
        } = config;
        let buffer = PersistentBuffer::create(buffer, (device, allocator))?;
        Ok(UniformBufferTypeErased {
            len,
            buffer,
            item_type_id,
            _phantom: PhantomData,
        })
    }
}

impl<O: Operation, A: Allocator> Destroy for UniformBufferTypeErased<O, A> {
    type Context<'a> = (&'a Device, &'a RefCell<&'a mut A>);
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.buffer.destroy(context)?;
        Ok(())
    }
}

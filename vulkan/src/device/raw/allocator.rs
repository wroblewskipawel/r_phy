mod linear;
mod page;
mod unpooled;

pub use linear::*;
pub use page::*;
pub use unpooled::*;

use std::{
    any::type_name, collections::HashMap, convert::Infallible, fmt::Debug, marker::PhantomData,
};

use ash::vk;
use type_kit::{
    Create, CreateResult, Destroy, DestroyResult, DropGuardError, FromGuard, GenIndexRaw,
    GuardCollection, GuardIndex, ScopedEntry, ScopedEntryResult, ScopedInnerMut, ScopedInnerRef,
    TypeGuard, TypeGuardCollection, TypedIndex, Valid,
};

use crate::{
    device::{
        memory::{DeviceLocal, HostCoherent, HostVisible, MemoryProperties},
        resources::buffer::ByteRange,
    },
    error::{AllocatorError, AllocatorResult, ResourceResult},
    Context,
};

use super::resources::{
    memory::{Memory, MemoryRaw},
    ResourceIndex,
};

pub struct Allocation<M: MemoryProperties> {
    range: ByteRange,
    memory: ResourceIndex<Memory<M>>,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> Debug for Allocation<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Allocation")
            .field("range", &self.range)
            .field("memory", &self.memory)
            .field("memory_type", &type_name::<M>())
            .finish()
    }
}

impl<M: MemoryProperties> Allocation<M> {
    #[inline]
    pub fn new(memory: ResourceIndex<Memory<M>>, range: ByteRange) -> Self {
        Self {
            range,
            memory,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationRaw {
    range: ByteRange,
    memory: TypeGuard<GenIndexRaw>,
}

impl<M: MemoryProperties> From<Valid<Allocation<M>>> for Allocation<M> {
    fn from(value: Valid<Allocation<M>>) -> Self {
        let AllocationRaw { range, memory } = value.into_inner();
        let memory: Valid<ResourceIndex<Memory<M>>> = memory.try_into().unwrap();
        let memory = memory.into();
        Self {
            range,
            memory,
            _phantom: PhantomData,
        }
    }
}

impl<M: MemoryProperties> FromGuard for Allocation<M> {
    type Inner = AllocationRaw;

    fn into_inner(self) -> Self::Inner {
        AllocationRaw {
            range: self.range,
            memory: self.memory.into_guard(),
        }
    }
}

#[derive(Debug, Default)]
struct MemoryMap {
    usage: HashMap<TypeGuard<GenIndexRaw>, usize>,
    memory: GuardCollection<MemoryRaw>,
}

impl MemoryMap {
    #[inline]
    fn new() -> Self {
        Self {
            usage: HashMap::default(),
            memory: GuardCollection::default(),
        }
    }

    #[inline]
    fn register<M: MemoryProperties>(&mut self, allocation: &Allocation<M>) {
        let memory = allocation.memory.clone().into_guard();
        *self.usage.entry(memory).or_default() += 1;
    }

    #[inline]
    fn pop<M: MemoryProperties>(
        &mut self,
        allocation: Allocation<M>,
    ) -> AllocatorResult<Option<ResourceIndex<Memory<M>>>> {
        let memory = allocation.memory.clone().into_guard();
        let count = self
            .usage
            .get_mut(&memory)
            .ok_or(AllocatorError::InvalidAllocationIndex)?;
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.usage.remove(&memory);
            Ok(Some(allocation.memory))
        } else {
            Ok(None)
        }
    }

    fn drain<M: MemoryProperties>(&mut self) -> Vec<ResourceIndex<Memory<M>>> {
        let (valid, rest): (Vec<_>, Vec<_>) = self
            .usage
            .drain()
            .map(|(memory, count)| {
                ResourceIndex::<Memory<M>>::try_from_guard(memory)
                    .map_err(|(memory, _)| (memory, count))
            })
            .partition(Result::is_ok);
        self.usage = rest.into_iter().map(Result::unwrap_err).collect();
        valid.into_iter().map(Result::unwrap).collect()
    }

    #[inline]
    fn free_memory_type<M: MemoryProperties>(&mut self, context: &Context) {
        for memory in self.drain::<M>().into_iter() {
            context.destroy_resource(memory).unwrap();
        }
    }
}

impl Drop for MemoryMap {
    #[inline]
    fn drop(&mut self) {
        assert!(self.usage.is_empty());
    }
}

#[derive(Debug)]
pub enum AllocatorState {
    Empty(()),
    Page(PageState),
    Linear(LinearState),
}

impl From<()> for AllocatorState {
    fn from(config: ()) -> Self {
        Self::Empty(config)
    }
}

impl State for () {
    fn try_get(config: &AllocatorState) -> Result<&Self, AllocatorError> {
        match config {
            AllocatorState::Empty(empty) => Ok(&empty),
            _ => Err(AllocatorError::InvalidConfiguration),
        }
    }
}

pub struct Allocator<S: Strategy> {
    inner: AllocatorInner,
    _phantom: PhantomData<S>,
}

pub trait State: Into<AllocatorState> {
    fn try_get(config: &AllocatorState) -> Result<&Self, AllocatorError>;
}

#[derive(Debug)]
pub struct AllocatorInner {
    allocations: TypeGuardCollection<AllocationRaw>,
    memory_map: MemoryMap,
    state: AllocatorState,
}

impl AllocatorInner {
    #[inline]
    pub fn new<S: Strategy>(state: S::State) -> Self {
        Self {
            allocations: TypeGuardCollection::default(),
            memory_map: MemoryMap::new(),
            state: state.into(),
        }
    }
}

impl Destroy for AllocatorInner {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.memory_map.free_memory_type::<DeviceLocal>(context);
        self.memory_map.free_memory_type::<HostVisible>(context);
        self.memory_map.free_memory_type::<HostCoherent>(context);
        Ok(())
    }
}

impl<S: Strategy> From<Valid<Allocator<S>>> for Allocator<S> {
    fn from(value: Valid<Allocator<S>>) -> Self {
        Self {
            inner: value.into_inner(),
            _phantom: PhantomData,
        }
    }
}

impl<S: Strategy> FromGuard for Allocator<S> {
    type Inner = AllocatorInner;

    fn into_inner(self) -> Self::Inner {
        self.inner
    }
}

pub struct AllocationRequest<M: MemoryProperties> {
    requirements: vk::MemoryRequirements,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> AllocationRequest<M> {
    #[inline]
    pub fn new(requirements: vk::MemoryRequirements) -> Self {
        Self {
            requirements,
            _phantom: PhantomData,
        }
    }
}

pub trait Strategy: 'static + Sized {
    type State: State;
    type CreateConfig<'a>: Into<Self::State>;

    fn allocate<'a, M: MemoryProperties>(
        allocator: ScopedInnerMut<'a, Allocator<Self>>,
        context: &Context,
        req: AllocationRequest<M>,
    ) -> ResourceResult<AllocationIndex<M>>;

    fn free<'a, M: MemoryProperties>(
        allocator: ScopedInnerMut<'a, Allocator<Self>>,
        context: &Context,
        allocation: AllocationIndex<M>,
    ) -> ResourceResult<()>;
}

impl<S: Strategy> Allocator<S> {
    #[inline]
    pub fn new(config: S::State) -> Self {
        Self {
            inner: AllocatorInner::new::<S>(config),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn access<'a, M: MemoryProperties>(
        &'a self,
        index: AllocationIndex<M>,
    ) -> ScopedEntryResult<'a, Allocation<M>> {
        self.inner
            .allocations
            .entry(TypedIndex::<Allocation<M>>::new(index))
    }
}

impl<S: Strategy> Create for Allocator<S> {
    type Config<'a> = S::CreateConfig<'a>;
    type CreateError = AllocatorError;

    fn create<'a, 'b>(config: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
        Ok(Self::new(config.into()))
    }
}

impl<S: Strategy> Destroy for Allocator<S> {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.inner.destroy(context)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BindResource {
    Image(vk::Image),
    Buffer(vk::Buffer),
}

impl From<vk::Image> for BindResource {
    #[inline]
    fn from(value: vk::Image) -> Self {
        Self::Image(value)
    }
}

impl From<vk::Buffer> for BindResource {
    #[inline]
    fn from(value: vk::Buffer) -> Self {
        Self::Buffer(value)
    }
}

impl Context {
    pub fn get_memory_type_index<M: MemoryProperties>(
        &self,
        req: &AllocationRequest<M>,
    ) -> AllocatorResult<u32> {
        let memory_type_bits = req.requirements.memory_type_bits;
        let memory_properties = M::properties();

        self.physical_device
            .properties
            .memory
            .memory_types
            .iter()
            .zip(0u32..)
            .find_map(|(memory, type_index)| {
                if (1 << type_index & memory_type_bits == 1 << type_index)
                    && memory.property_flags.contains(memory_properties)
                {
                    Some(type_index)
                } else {
                    None
                }
            })
            .ok_or(AllocatorError::UnsupportedMemoryType)
    }

    // pub fn bind_resource_memory<R: Into<BindResource>, S: Strategy, M: MemoryProperties>(
    //     &self,
    //     resource: R,
    //     allocation: AllocationEntry<S, M>,
    // ) -> ResourceResult<()> {
    //     let AllocationEntry { allocation, allocator } = allocation;
    //     let allocator = self.
    //     let resource = resource.into();
    //     let memory = self
    //         .resources
    //         .memory
    //         .inner(allocation.allocation.memory)
    //         .unwrap();
    //     match resource {
    //         BindResource::Image(image) => unsafe {
    //             self.device
    //                 .bind_image_memory(image, memory.memory, memory.range.start)?;
    //         },
    //         BindResource::Buffer(buffer) => unsafe {
    //             self.device
    //                 .bind_buffer_memory(buffer, memory.memory, memory.range.start)?;
    //         },
    //     }
    //     Ok(())
    // }
}

pub type AllocatorIndex<T> = GuardIndex<Allocator<T>>;
pub type AllocationIndex<T> = GuardIndex<Allocation<T>>;

#[derive(Debug)]
pub struct AllocationEntry<S: Strategy, M: MemoryProperties> {
    allocator: AllocatorIndex<S>,
    allocation: AllocationIndex<M>,
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationEntryRaw {
    allocator: TypeGuard<GenIndexRaw>,
    allocation: TypeGuard<GenIndexRaw>,
}

impl<S: Strategy, M: MemoryProperties> From<Valid<AllocationEntry<S, M>>>
    for AllocationEntry<S, M>
{
    #[inline]
    fn from(value: Valid<AllocationEntry<S, M>>) -> Self {
        let AllocationEntryRaw {
            allocator,
            allocation,
        } = value.into_inner();
        Self {
            allocator: {
                let allocator: Valid<AllocatorIndex<S>> = allocator.try_into().unwrap();
                allocator.into()
            },
            allocation: {
                let allocation: Valid<AllocationIndex<M>> = allocation.try_into().unwrap();
                allocation.into()
            },
        }
    }
}

impl<S: Strategy, M: MemoryProperties> FromGuard for AllocationEntry<S, M> {
    type Inner = AllocationEntryRaw;

    #[inline]
    fn into_inner(self) -> Self::Inner {
        AllocationEntryRaw {
            allocator: self.allocator.into_guard(),
            allocation: self.allocation.into_guard(),
        }
    }
}

pub struct AllocatorStorage {
    allocators: GuardCollection<AllocatorInner>,
}

impl AllocatorStorage {
    #[inline]
    pub fn new() -> Self {
        Self {
            allocators: GuardCollection::default(),
        }
    }

    #[inline]
    pub fn create_allocator<'a, 'b, S: Strategy>(
        &mut self,
        context: &'a Context,
        config: S::CreateConfig<'b>,
    ) -> ResourceResult<AllocatorIndex<S>> {
        let allocator = Allocator::<S>::create(config, context)?;
        let index = self.allocators.push(allocator.into_guard())?;
        Ok(index)
    }

    #[inline]
    pub fn destroy_allocator<S: Strategy>(
        &mut self,
        context: &Context,
        index: AllocatorIndex<S>,
    ) -> ResourceResult<()> {
        let _ = self.allocators.pop(index)?.destroy(context);
        Ok(())
    }

    #[inline]
    pub fn allocate<M: MemoryProperties, S: Strategy>(
        &mut self,
        context: &Context,
        index: AllocatorIndex<S>,
        req: AllocationRequest<M>,
    ) -> ResourceResult<AllocationEntry<S, M>> {
        let allocator = self.allocators.inner_mut(index.clone())?;
        let allocation = S::allocate(allocator, context, req)?;
        let entry = AllocationEntry {
            allocator: index,
            allocation,
        };
        Ok(entry)
    }

    #[inline]
    pub fn free<M: MemoryProperties, S: Strategy>(
        &mut self,
        context: &Context,
        index: AllocationEntry<S, M>,
    ) -> ResourceResult<()> {
        let allocator = self.allocators.inner_mut(index.allocator)?;
        S::free::<M>(allocator, context, index.allocation)?;
        Ok(())
    }

    // #[inline]
    // pub fn access<'a, S: Strategy, M: MemoryProperties>(
    //     &'a self,
    //     index: AllocationEntry<S, M>,
    // ) -> ScopedEntryResult<'a, Allocation<M>> {
    //     let allocator: ScopedInnerRef<Allocator<S>> = self.allocators.inner_ref(index.allocator)?;
    //     allocator.allocations.entry(index.allocation)
    // }
}

impl Destroy for AllocatorStorage {
    type Context<'a> = &'a Context;
    type DestroyError = DropGuardError<Infallible>;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.allocators.destroy(context)?;
        Ok(())
    }
}

use std::convert::Infallible;

use type_kit::{
    Create, Destroy, DestroyResult, FromGuard, GenCollection, GenIndexRaw, TypeGuard, Valid,
};

use crate::{
    device::{
        memory::MemoryProperties,
        raw::resources::{memory::Memory, ResourceIndex},
        resources::buffer::ByteRange,
    },
    error::AllocatorError,
};

use super::{AllocatorContext, AllocatorState, State, Strategy};

pub struct LinearBuffer<M: MemoryProperties> {
    memory: ResourceIndex<Memory<M>>,
    range: ByteRange,
}

pub struct LinearBufferRaw {
    memory: TypeGuard<GenIndexRaw>,
    range: ByteRange,
}

impl<M: MemoryProperties> From<Valid<LinearBuffer<M>>> for LinearBuffer<M> {
    #[inline]
    fn from(value: Valid<LinearBuffer<M>>) -> Self {
        let LinearBufferRaw { memory, range } = value.into_inner();
        let memory: Valid<ResourceIndex<Memory<M>>> = memory.try_into().unwrap();
        let memory = memory.into();
        Self { memory, range }
    }
}

impl<M: MemoryProperties> FromGuard for LinearBuffer<M> {
    type Inner = LinearBufferRaw;

    #[inline]
    fn into_inner(self) -> Self::Inner {
        LinearBufferRaw {
            memory: self.memory.into_guard(),
            range: self.range,
        }
    }
}

pub struct LinearConfig {}

impl From<LinearConfig> for LinearState {
    #[inline]
    fn from(_: LinearConfig) -> Self {
        LinearState {}
    }
}

#[derive(Debug)]
pub struct LinearState {}

impl From<LinearState> for AllocatorState {
    #[inline]
    fn from(state: LinearState) -> Self {
        AllocatorState::Linear(state)
    }
}

impl State for LinearState {
    #[inline]
    fn try_get(state: &AllocatorState) -> Result<&Self, AllocatorError> {
        match state {
            AllocatorState::Linear(config) => Ok(config),
            _ => Err(AllocatorError::InvalidConfiguration),
        }
    }
}

pub struct Linear {
    buffers: GenCollection<LinearBufferRaw>,
}

impl Linear {
    #[inline]
    pub fn new() -> Self {
        Self {
            buffers: GenCollection::default(),
        }
    }
}

impl Create for Linear {
    type Config<'a> = ();
    type CreateError = AllocatorError;

    #[inline]
    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        todo!()
    }
}

impl Destroy for Linear {
    type Context<'a> = AllocatorContext<'a>;
    type DestroyError = Infallible;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        todo!()
    }
}

impl Strategy for Linear {
    type State = LinearState;
    type CreateConfig<'a> = LinearConfig;

    fn allocate<'a, M: MemoryProperties>(
        allocator: type_kit::ScopedInnerMut<'a, super::Allocator<Self>>,
        context: &crate::Context,
        req: super::AllocationRequest<M>,
    ) -> crate::error::AllocatorResult<super::AllocationIndex<M>> {
        todo!()
    }

    fn free<'a, M: MemoryProperties>(
        allocator: type_kit::ScopedInnerMut<'a, super::Allocator<Self>>,
        context: &crate::Context,
        allocation: super::AllocationIndex<M>,
    ) -> crate::error::AllocatorResult<()> {
        todo!()
    }
}

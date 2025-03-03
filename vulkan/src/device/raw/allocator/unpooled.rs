use std::convert::Infallible;

use type_kit::{Create, Destroy, DestroyResult, FromGuard, ScopedInnerMut};

use crate::{
    device::{
        memory::MemoryProperties,
        raw::resources::{
            memory::{Memory, MemoryAllocateInfo},
            ResourceIndex,
        },
        resources::buffer::ByteRange,
    },
    error::{AllocatorError, ResourceResult},
    Context,
};

use super::{Allocation, AllocationIndex, AllocationRequest, Allocator, Strategy};

pub struct Unpooled {}

impl Create for Unpooled {
    type Config<'a> = ();
    type CreateError = AllocatorError;

    fn create<'a, 'b>(_: Self::Config<'a>, _: Self::Context<'b>) -> type_kit::CreateResult<Self> {
        Ok(Unpooled {})
    }
}

impl Destroy for Unpooled {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, _: Self::Context<'a>) -> DestroyResult<Self> {
        Ok(())
    }
}

impl Strategy for Unpooled {
    type State = ();
    type CreateConfig<'a> = ();

    fn allocate<'a, M: MemoryProperties>(
        mut allocator: ScopedInnerMut<'a, Allocator<Self>>,
        context: &Context,
        req: AllocationRequest<M>,
    ) -> ResourceResult<AllocationIndex<M>> {
        let alloc_info = MemoryAllocateInfo::new()
            .with_allocation_size(req.requirements.size)
            .with_memory_type_index(context.get_memory_type_index(&req)?);
        let memory: ResourceIndex<Memory<M>> = context.create_resource(alloc_info)?;
        let range = ByteRange::new(req.requirements.size as usize);
        let allocation = Allocation::new(memory, range);
        allocator.memory_map.register(&allocation);
        let index = allocator.allocations.push(allocation.into_guard())?;
        Ok(index)
    }

    fn free<'a, M: MemoryProperties>(
        mut allocator: type_kit::ScopedInnerMut<'a, Allocator<Self>>,
        context: &Context,
        allocation: super::AllocationIndex<M>,
    ) -> ResourceResult<()> {
        let allocation = Allocation::<M>::try_from_guard(allocator.allocations.pop(allocation)?)
            .map_err(|(_, err)| err)?;
        let memory = allocator.memory_map.pop(allocation)?;
        if let Some(memory) = memory {
            context.destroy_resource(memory)?;
        }
        Ok(())
    }
}

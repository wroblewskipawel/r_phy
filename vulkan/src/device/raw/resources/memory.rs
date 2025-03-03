use std::{
    any::type_name,
    convert::Infallible,
    ffi::c_void,
    fmt::{Debug, Formatter},
    marker::PhantomData,
};

use ash::vk;
use type_kit::{Create, Destroy, DestroyResult, FromGuard, TypeGuardUnlocked};

use crate::{device::memory::MemoryProperties, error::ResourceError, Context};

use super::Resource;

#[derive(Debug, Clone, Copy)]
pub struct MemoryAllocateInfo<M: MemoryProperties> {
    info: vk::MemoryAllocateInfo,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> Default for MemoryAllocateInfo<M> {
    #[inline]
    fn default() -> Self {
        Self {
            info: vk::MemoryAllocateInfo::default(),
            _phantom: PhantomData,
        }
    }
}

impl<M: MemoryProperties> MemoryAllocateInfo<M> {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_memory_type_index(mut self, memory_type_index: u32) -> Self {
        self.info.memory_type_index = memory_type_index;
        self
    }

    #[inline]
    pub fn with_allocation_size(mut self, allocation_size: vk::DeviceSize) -> Self {
        self.info.allocation_size = allocation_size;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryRaw {
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    type_index: u32,
    ptr: Option<*mut c_void>,
}

pub struct Memory<M: MemoryProperties> {
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    type_index: u32,
    ptr: Option<*mut c_void>,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> Clone for Memory<M> {
    fn clone(&self) -> Self {
        Self {
            memory: self.memory,
            size: self.size,
            type_index: self.type_index,
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<M: MemoryProperties> Debug for Memory<M> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("Memory")
            .field("memory", &self.memory)
            .field("size", &self.size)
            .field("ptr", &self.ptr)
            .field("memory_type", &type_name::<M>())
            .finish()
    }
}

impl<M: MemoryProperties> Create for Memory<M> {
    type Config<'a> = MemoryAllocateInfo<M>;
    type CreateError = ResourceError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let MemoryAllocateInfo { info, .. } = config;
        let memory = Memory {
            memory: unsafe { context.allocate_memory(&info, None)? },
            size: info.allocation_size,
            type_index: info.memory_type_index,
            ptr: None,
            _phantom: PhantomData,
        };
        Ok(memory)
    }
}

impl Destroy for MemoryRaw {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        unsafe {
            context.free_memory(self.memory, None);
        }
        Ok(())
    }
}

impl<M: MemoryProperties> Destroy for Memory<M> {
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        unsafe {
            context.free_memory(self.memory, None);
        }
        Ok(())
    }
}

impl<M: MemoryProperties> From<TypeGuardUnlocked<MemoryRaw, Memory<M>>> for Memory<M> {
    fn from(value: TypeGuardUnlocked<MemoryRaw, Memory<M>>) -> Self {
        let MemoryRaw {
            memory,
            size,
            type_index,
            ptr,
        } = value.into_inner();
        Self {
            memory,
            size,
            type_index,
            ptr,
            _phantom: PhantomData,
        }
    }
}

impl<M: MemoryProperties> FromGuard for Memory<M> {
    type Inner = MemoryRaw;

    fn into_inner(self) -> Self::Inner {
        MemoryRaw {
            memory: self.memory,
            size: self.size,
            type_index: self.type_index,
            ptr: self.ptr,
        }
    }
}

impl<M: MemoryProperties> Resource for Memory<M> {
    type RawType = MemoryRaw;
}

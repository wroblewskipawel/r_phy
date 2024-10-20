mod default;
mod page;
mod r#static;

use std::{
    any::{type_name, TypeId},
    error::Error,
    fmt::Debug,
    marker::PhantomData,
};

use ash::vk::{self, PhysicalDeviceMemoryProperties};
pub use default::*;
#[allow(unused_imports)]
pub use page::*;
pub use r#static::*;

use crate::{device::Device, error::AllocResult};

use super::{DeviceLocal, HostCoherent, HostVisible, Memory, MemoryProperties, Resource};

pub trait AllocatorCreate: Sized + 'static {
    type Config;

    fn create(device: &Device, config: &Self::Config) -> Result<Self, Box<dyn Error>>;
    fn destroy(&mut self, device: &Device);
}

pub trait Allocator: AllocatorCreate {
    type Allocation<M: MemoryProperties>: Memory;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        request: AllocReqTyped<M>,
    ) -> AllocResult<Self::Allocation<M>>;

    fn free<M: MemoryProperties>(&mut self, device: &Device, allocation: &mut Self::Allocation<M>);
}

#[derive(Debug)]
pub enum AllocReq {
    HostVisible(AllocReqTyped<HostVisible>),
    DeviceLocal(AllocReqTyped<DeviceLocal>),
    HostCoherent(AllocReqTyped<HostCoherent>),
}

impl<M: MemoryProperties> From<AllocReqTyped<M>> for AllocReq {
    fn from(value: AllocReqTyped<M>) -> AllocReq {
        let type_id = TypeId::of::<M>();
        if type_id == TypeId::of::<HostVisible>() {
            AllocReq::HostVisible(AllocReqTyped {
                requirements: value.requirements,
                _phantom: PhantomData,
            })
        } else if type_id == TypeId::of::<DeviceLocal>() {
            AllocReq::DeviceLocal(AllocReqTyped {
                requirements: value.requirements,
                _phantom: PhantomData,
            })
        } else if type_id == TypeId::of::<HostCoherent>() {
            AllocReq::HostCoherent(AllocReqTyped {
                requirements: value.requirements,
                _phantom: PhantomData,
            })
        } else {
            unreachable!();
        }
    }
}

impl AllocReq {
    fn requirements(&self) -> vk::MemoryRequirements {
        match self {
            AllocReq::HostVisible(req) => req.requirements,
            AllocReq::DeviceLocal(req) => req.requirements,
            AllocReq::HostCoherent(req) => req.requirements,
        }
    }

    fn contained_type_id(&self) -> TypeId {
        match self {
            AllocReq::HostVisible(_) => TypeId::of::<HostVisible>(),
            AllocReq::DeviceLocal(_) => TypeId::of::<DeviceLocal>(),
            AllocReq::HostCoherent(_) => TypeId::of::<HostCoherent>(),
        }
    }

    pub fn get_memory_type_index(
        &self,
        properties: &PhysicalDeviceMemoryProperties,
    ) -> Option<u32> {
        match self {
            AllocReq::HostVisible(req) => req.get_memory_type_index(properties),
            AllocReq::DeviceLocal(req) => req.get_memory_type_index(properties),
            AllocReq::HostCoherent(req) => req.get_memory_type_index(properties),
        }
    }

    pub fn properties(&self) -> vk::MemoryPropertyFlags {
        match self {
            AllocReq::HostVisible(_) => HostVisible::properties(),
            AllocReq::DeviceLocal(_) => DeviceLocal::properties(),
            AllocReq::HostCoherent(_) => HostCoherent::properties(),
        }
    }
}

#[derive(Debug)]
pub struct AllocReqTyped<T: MemoryProperties> {
    requirements: vk::MemoryRequirements,
    _phantom: PhantomData<T>,
}

impl<M: MemoryProperties> TryFrom<AllocReq> for AllocReqTyped<M> {
    type Error = Box<dyn Error>;

    fn try_from(value: AllocReq) -> Result<Self, Self::Error> {
        if value.contained_type_id() == TypeId::of::<M>() {
            Ok(Self {
                requirements: value.requirements(),
                _phantom: PhantomData,
            })
        } else {
            Err(format!(
                "Invalid memory type cast {:?} as {}",
                value,
                type_name::<M>()
            )
            .into())
        }
    }
}

impl<T: MemoryProperties> Clone for AllocReqTyped<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: MemoryProperties> Copy for AllocReqTyped<T> {}

impl<M: MemoryProperties> AllocReqTyped<M> {
    pub fn get_memory_type_index(
        &self,
        properties: &PhysicalDeviceMemoryProperties,
    ) -> Option<u32> {
        let memory_type_bits = self.requirements.memory_type_bits;
        let memory_properties = M::properties();

        properties
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
    }
}

impl Device {
    pub fn get_alloc_req<T: Into<Resource>, M: MemoryProperties>(
        &self,
        resource: T,
    ) -> AllocReqTyped<M> {
        let requirements = match resource.into() {
            Resource::Buffer(buffer) => unsafe { self.get_buffer_memory_requirements(buffer) },
            Resource::Image(image) => unsafe { self.get_image_memory_requirements(image) },
        };
        AllocReqTyped {
            requirements,
            _phantom: PhantomData,
        }
    }
}

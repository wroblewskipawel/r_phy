mod default;
mod page;
mod r#static;

use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

use ash::vk::{self, PhysicalDeviceMemoryProperties};
pub use default::*;
#[allow(unused_imports)]
pub use page::*;
pub use r#static::*;

use crate::renderer::vulkan::device::VulkanDevice;

use super::{Memory, MemoryProperties, Resource};

#[derive(Debug, Clone, Copy)]
pub enum DeviceAllocError {
    OutOfMemory,
    UnsupportedMemoryType,
    VulkanError(vk::Result),
}

impl Display for DeviceAllocError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            DeviceAllocError::OutOfMemory => write!(f, "Out of memory"),
            DeviceAllocError::UnsupportedMemoryType => write!(f, "Unsupported memory type"),
            DeviceAllocError::VulkanError(err) => write!(f, "Vulkan error: {}", err),
        }
    }
}

impl From<vk::Result> for DeviceAllocError {
    fn from(err: vk::Result) -> Self {
        DeviceAllocError::VulkanError(err)
    }
}

impl Error for DeviceAllocError {}

pub trait AllocatorCreate: Sized + 'static {
    type Config;

    fn create(device: &VulkanDevice, config: &Self::Config) -> Result<Self, Box<dyn Error>>;
    fn destroy(&mut self, device: &VulkanDevice);
}

pub trait Allocator: AllocatorCreate {
    type Allocation<M: MemoryProperties>: Memory<M>;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &VulkanDevice,
        request: AllocReq<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError>;

    fn free<M: MemoryProperties>(
        &mut self,
        device: &VulkanDevice,
        allocation: &mut Self::Allocation<M>,
    );
}

#[derive(Debug)]
pub struct AllocReq<T: MemoryProperties> {
    requirements: vk::MemoryRequirements,
    _phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct AllocReqRaw {
    requirements: vk::MemoryRequirements,
    properties: vk::MemoryPropertyFlags,
}

impl<T: MemoryProperties> From<AllocReq<T>> for AllocReqRaw {
    fn from(value: AllocReq<T>) -> Self {
        Self {
            requirements: value.requirements,
            properties: T::properties(),
        }
    }
}

impl<T: MemoryProperties> Clone for AllocReq<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: MemoryProperties> Copy for AllocReq<T> {}

impl AllocReqRaw {
    pub fn get_memory_type_index(
        &self,
        properties: &PhysicalDeviceMemoryProperties,
    ) -> Option<u32> {
        let memory_type_bits = self.requirements.memory_type_bits;
        let memory_properties = self.properties;

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

impl<M: MemoryProperties> AllocReq<M> {
    pub fn get_memory_type_index(
        &self,
        properties: &PhysicalDeviceMemoryProperties,
    ) -> Option<u32> {
        let raw: AllocReqRaw = (*self).into();
        raw.get_memory_type_index(properties)
    }
}

impl VulkanDevice {
    pub fn get_alloc_req<T: Into<Resource>, M: MemoryProperties>(
        &self,
        resource: T,
    ) -> AllocReq<M> {
        let requirements = match resource.into() {
            Resource::Buffer(buffer) => unsafe { self.get_buffer_memory_requirements(buffer) },
            Resource::Image(image) => unsafe { self.get_image_memory_requirements(image) },
        };
        AllocReq {
            requirements,
            _phantom: PhantomData,
        }
    }
}

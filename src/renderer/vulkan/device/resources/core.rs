use std::error::Error;

use crate::renderer::vulkan::device::{
    memory::{AllocReq, Allocator, MemoryProperties},
    VulkanDevice,
};

pub mod buffer;
pub mod image;

pub trait Partial {
    type Memory: MemoryProperties;
    fn requirements(&self) -> AllocReq<Self::Memory>;
}

pub trait PartialBuilder {
    type Partial: Partial;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>>;
}

pub trait FromPartial: Sized {
    type Partial<'a>: Partial;
    type Allocator: Allocator;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>>;
}

use std::error::Error;

use crate::renderer::vulkan::device::{
    memory::{AllocReq, Allocator},
    VulkanDevice,
};

pub mod buffer;
pub mod image;

pub trait Partial {
    fn requirements(&self) -> impl Iterator<Item = AllocReq>;
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

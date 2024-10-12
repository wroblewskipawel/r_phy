use std::error::Error;

use crate::device::{
    memory::{AllocReq, Allocator},
    VulkanDevice,
};

pub mod buffer;
pub mod image;

pub trait PartialBuilder<'a>: Sized {
    type Config;
    type Target<A: Allocator>;

    fn prepare(config: Self::Config, device: &VulkanDevice) -> Result<Self, Box<dyn Error>>;
    fn requirements(&self) -> impl Iterator<Item = AllocReq>;
    fn finalize<A: Allocator>(
        self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>>;
}

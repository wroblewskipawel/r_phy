use crate::{
    device::{
        memory::{AllocReq, Allocator},
        Device,
    },
    error::VkResult,
};

pub mod buffer;
pub mod image;

pub trait PartialBuilder<'a>: Sized {
    type Config;
    type Target<A: Allocator>;

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self>;
    fn requirements(&self) -> impl Iterator<Item = AllocReq>;
}

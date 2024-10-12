use std::error::Error;

use crate::device::{
    memory::{AllocReq, Allocator},
    Device,
};

pub mod buffer;
pub mod image;

pub trait PartialBuilder<'a>: Sized {
    type Config;
    type Target<A: Allocator>;

    fn prepare(config: Self::Config, device: &Device) -> Result<Self, Box<dyn Error>>;
    fn requirements(&self) -> impl Iterator<Item = AllocReq>;
    fn finalize<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>>;
}

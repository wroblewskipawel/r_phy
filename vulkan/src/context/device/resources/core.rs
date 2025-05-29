use std::{cell::RefCell, convert::Infallible, marker::PhantomData};

use type_kit::{Create, CreateResult, Destroy, DestroyResult};

use crate::context::{
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

pub struct DummyPack<A: Allocator> {
    _phantom: PhantomData<A>,
}

impl<A: Allocator> Create for DummyPack<A> {
    type Config<'a> = ();
    type CreateError = Infallible;

    fn create<'a, 'b>(_: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
        unreachable!()
    }
}

impl<A: Allocator> Destroy for DummyPack<A> {
    type Context<'a> = (&'a Device, &'a RefCell<&'a mut A>);
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, _: Self::Context<'a>) -> DestroyResult<Self> {
        unreachable!()
    }
}

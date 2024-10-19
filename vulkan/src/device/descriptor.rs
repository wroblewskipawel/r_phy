mod layout;
mod presets;
mod writer;

use std::{
    any::{type_name, TypeId},
    error::Error,
    marker::PhantomData,
};

pub use layout::*;
pub use presets::*;
use type_kit::Destroy;
pub use writer::*;

use ash::vk;

use super::{
    pipeline::{GraphicsPipeline, GraphicsPipelineConfig, Layout},
    Device,
};

#[derive(Debug)]
pub struct DescriptorPoolData {
    pool: vk::DescriptorPool,
    sets: Vec<vk::DescriptorSet>,
}

#[derive(Debug)]
pub struct DescriptorPool<T: DescriptorLayout> {
    data: DescriptorPoolData,
    _phantom: PhantomData<T>,
}

impl<'a, T: DescriptorLayout> From<&'a DescriptorPool<T>> for &'a DescriptorPoolData {
    fn from(pool: &'a DescriptorPool<T>) -> Self {
        &pool.data
    }
}

impl<'a, T: DescriptorLayout> From<&'a mut DescriptorPool<T>> for &'a mut DescriptorPoolData {
    fn from(pool: &'a mut DescriptorPool<T>) -> Self {
        &mut pool.data
    }
}

#[derive(Debug)]
pub struct Descriptor<T: DescriptorLayout> {
    set: vk::DescriptorSet,
    _phantom: PhantomData<T>,
}

impl<T: DescriptorLayout> Clone for Descriptor<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DescriptorLayout> Copy for Descriptor<T> {}

impl<T: DescriptorLayout> From<Descriptor<T>> for vk::DescriptorSet {
    fn from(descriptor: Descriptor<T>) -> Self {
        descriptor.set
    }
}

impl<T: DescriptorLayout> DescriptorPool<T> {
    pub fn get(&self, index: usize) -> Descriptor<T> {
        Descriptor {
            set: self.data.sets[index],
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.sets.len()
    }
}

#[derive(Debug)]
pub struct DescriptorPoolRef<'a, T: DescriptorLayout> {
    data: &'a DescriptorPoolData,
    _phantom: PhantomData<T>,
}

impl<'a, T: DescriptorLayout, N: DescriptorLayout> TryFrom<&'a DescriptorPool<T>>
    for DescriptorPoolRef<'a, N>
{
    type Error = &'static str;

    fn try_from(pool: &'a DescriptorPool<T>) -> Result<DescriptorPoolRef<'a, N>, Self::Error> {
        if TypeId::of::<T>() == TypeId::of::<N>() {
            Ok(DescriptorPoolRef {
                data: &pool.data,
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid DescriptorLayout type")
        }
    }
}

impl<'a, T: DescriptorLayout> DescriptorPoolRef<'a, T> {
    pub fn get(&self, index: usize) -> Descriptor<T> {
        Descriptor {
            set: self.data.sets[index],
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct DescriptorBindingData {
    pub set_index: u32,
    pub set: vk::DescriptorSet,
    pub pipeline_layout: vk::PipelineLayout,
}

impl<T: DescriptorLayout> Descriptor<T> {
    pub fn get_binding_data<C: GraphicsPipelineConfig>(
        &self,
        pipeline: &GraphicsPipeline<C>,
    ) -> Result<DescriptorBindingData, Box<dyn Error>> {
        let set_index = C::Layout::sets().get_set_index::<T>().unwrap_or_else(|| {
            panic!(
                "DescriptorSet {} not present in layout DescriptorSets {}",
                type_name::<T>(),
                type_name::<<C::Layout as Layout>::Descriptors>()
            )
        });
        Ok(DescriptorBindingData {
            set_index,
            set: self.set,
            pipeline_layout: pipeline.layout().into(),
        })
    }
}

impl Destroy for DescriptorPoolData {
    type Context<'a> = &'a Device;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        unsafe {
            context.destroy_descriptor_pool(self.pool, None);
        };
    }
}

impl<L: DescriptorLayout> Destroy for DescriptorPool<L> {
    type Context<'a> = &'a Device;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        self.data.destroy(context);
    }
}

use std::{
    any::{type_name, TypeId},
    error::Error,
    marker::PhantomData,
};

use ash::vk;

use crate::renderer::vulkan::device::pipeline::{GraphicsPipeline, GraphicsPipelineConfig, Layout};

use super::{Descriptor, DescriptorLayout, DescriptorPool};

pub struct DescriptorPoolTypeErased {
    type_id: TypeId,
    pool: vk::DescriptorPool,
    sets: Vec<vk::DescriptorSet>,
}

pub struct DescriptorPoolRef<'a, T: DescriptorLayout> {
    pool: &'a DescriptorPoolTypeErased,
    _phantom: PhantomData<T>,
}

impl<T: DescriptorLayout> From<DescriptorPool<T>> for DescriptorPoolTypeErased {
    fn from(pool: DescriptorPool<T>) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            pool: pool.pool,
            sets: pool
                .sets
                .into_iter()
                .map(|Descriptor { set, .. }| set)
                .collect(),
        }
    }
}

impl From<&mut DescriptorPoolTypeErased> for vk::DescriptorPool {
    fn from(pool: &mut DescriptorPoolTypeErased) -> Self {
        pool.pool
    }
}

impl<'a, T: DescriptorLayout> TryFrom<&'a DescriptorPoolTypeErased> for DescriptorPoolRef<'a, T> {
    type Error = &'static str;

    fn try_from(pool: &'a DescriptorPoolTypeErased) -> Result<Self, Self::Error> {
        if pool.type_id == TypeId::of::<T>() {
            Ok(Self {
                pool,
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
            set: self.pool.sets[index],
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.pool.sets.len()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DescriptorBindingData {
    pub set_index: u32,
    pub set: vk::DescriptorSet,
    pub pipeline_layout: vk::PipelineLayout,
}

impl<L: DescriptorLayout> Descriptor<L> {
    pub fn get_binding_data<C: GraphicsPipelineConfig>(
        &self,
        pipeline: &GraphicsPipeline<C>,
    ) -> Result<DescriptorBindingData, Box<dyn Error>> {
        let set_index = C::Layout::sets().get_set_index::<L>().unwrap_or_else(|| {
            panic!(
                "DescriptorSet {} not present in layout DescriptorSets {}",
                type_name::<L>(),
                type_name::<<C::Layout as Layout>::Descriptors>()
            )
        });
        Ok(DescriptorBindingData {
            set_index,
            set: self.set,
            pipeline_layout: pipeline.layout.layout,
        })
    }
}

use std::{any::TypeId, marker::PhantomData};

use ash::vk;

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

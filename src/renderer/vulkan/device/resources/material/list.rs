use std::error::Error;

use crate::{
    core::{Cons, Nil, TypedNil},
    renderer::{
        model::{MaterialCollection, MaterialTypeList},
        vulkan::device::{
            memory::{Allocator, HostCoherent, HostVisibleMemory},
            VulkanDevice,
        },
    },
};

use super::{MaterialPack, MaterialPackRef, VulkanMaterial};

pub trait MaterialPackList<A: Allocator>: MaterialTypeList {
    fn destroy(&mut self, device: &VulkanDevice, allocator: &mut A);

    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>>;
}

impl<A: Allocator> MaterialPackList<A> for TypedNil<A> {
    fn destroy(&mut self, _device: &VulkanDevice, _allocator: &mut A) {}
    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>> {
        None
    }
}

impl<A: Allocator, M: VulkanMaterial, N: MaterialPackList<A>> MaterialTypeList
    for Cons<Option<MaterialPack<M, A>>, N>
{
    const LEN: usize = N::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<A: Allocator, M: VulkanMaterial, N: MaterialPackList<A>> MaterialPackList<A>
    for Cons<Option<MaterialPack<M, A>>, N>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    fn destroy(&mut self, device: &VulkanDevice, allocator: &mut A) {
        if let Some(material_pack) = &mut self.head {
            device.destroy_material_pack(material_pack, allocator);
        }
        self.tail.destroy(device, allocator);
    }

    fn try_get<T: VulkanMaterial>(&self) -> Option<MaterialPackRef<T>> {
        self.head
            .as_ref()
            .and_then(|pack| pack.try_into().ok())
            .or_else(|| self.tail.try_get::<T>())
    }
}

pub trait MaterialPackListBuilder: MaterialTypeList + 'static {
    type Pack<A: Allocator>: MaterialPackList<A>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory;
    fn build<A: Allocator>(
        &self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory;
}

impl<M: VulkanMaterial, N: MaterialPackListBuilder> MaterialPackListBuilder for Cons<Vec<M>, N> {
    type Pack<A: Allocator> = Cons<Option<MaterialPack<Self::Item, A>>, N::Pack<A>> where A::Allocation<HostCoherent>: HostVisibleMemory;
    fn build<A: Allocator>(
        &self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let materials = self.get();
        Ok(Cons {
            head: if !materials.is_empty() {
                Some(device.load_material_pack(allocator, materials)?)
            } else {
                None
            },
            tail: self.next().build(device, allocator)?,
        })
    }
}

impl MaterialPackListBuilder for Nil {
    type Pack<A: Allocator> = TypedNil<A> where A::Allocation<HostCoherent>: HostVisibleMemory;

    fn build<A: Allocator>(
        &self,
        _device: &VulkanDevice,
        _allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        Ok(TypedNil::new())
    }
}

impl VulkanDevice {
    pub fn load_materials<A: Allocator, B: MaterialPackListBuilder>(
        &self,
        allocator: &mut A,
        material_types: &B,
    ) -> Result<B::Pack<A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        material_types.build(self, allocator)
    }

    pub fn destroy_materials<A: Allocator, M: MaterialPackList<A>>(
        &self,
        packs: &mut M,
        allocator: &mut A,
    ) {
        packs.destroy(self, allocator)
    }
}

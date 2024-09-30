use std::error::Error;

use crate::{
    core::{Cons, Nil, TypedNil},
    renderer::{
        model::{MaterialCollection, MaterialTypeList},
        vulkan::device::{
            memory::{AllocReq, Allocator, HostCoherent, HostVisibleMemory},
            VulkanDevice,
        },
    },
};

use super::{MaterialPack, MaterialPackPartial, MaterialPackRef, VulkanMaterial};

pub trait MaterialPackListBuilder: MaterialTypeList {
    type Pack<A: Allocator>: MaterialPackList<A>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn prepare<A: Allocator>(
        &self,
        device: &VulkanDevice,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;
}

impl MaterialPackListBuilder for Nil {
    type Pack<A: Allocator> = TypedNil<A>
    where
    <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn prepare<A: Allocator>(
        &self,
        _device: &VulkanDevice,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        Ok(Nil {})
    }
}

impl<M: VulkanMaterial, N: MaterialPackListBuilder> MaterialPackListBuilder for Cons<Vec<M>, N> {
    type Pack<A: Allocator> = Cons<Option<MaterialPack<M, A>>, N::Pack<A>>
    where
    <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn prepare<A: Allocator>(
        &self,
        device: &VulkanDevice,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let materials = self.get();
        let partial = if !materials.is_empty() {
            Some(device.prepare_material_pack(materials)?)
        } else {
            None
        };
        Ok(Cons {
            head: partial,
            tail: self.next().prepare(device)?,
        })
    }
}

pub trait MaterialPackListPartial: Sized {
    type Pack<A: Allocator>: MaterialPackList<A>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn get_memory_requirements(&self) -> Vec<AllocReq>;

    fn allocate<A: Allocator>(
        self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;
}

impl MaterialPackListPartial for Nil {
    type Pack<A: Allocator> = TypedNil<A> where <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn get_memory_requirements(&self) -> Vec<AllocReq> {
        vec![]
    }

    fn allocate<A: Allocator>(
        self,
        _device: &VulkanDevice,
        _allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        Ok(TypedNil::new())
    }
}

impl<'a, M: VulkanMaterial, N: MaterialPackListPartial> MaterialPackListPartial
    for Cons<Option<MaterialPackPartial<'a, M>>, N>
{
    type Pack<A: Allocator> = Cons<Option<MaterialPack<M, A>>, N::Pack<A>>
    where
    <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory;

    fn get_memory_requirements(&self) -> Vec<AllocReq> {
        let mut alloc_reqs = self.tail.get_memory_requirements();
        if let Some(partial) = &self.head {
            alloc_reqs.extend(partial.get_alloc_req());
        }
        alloc_reqs
    }

    fn allocate<A: Allocator>(
        self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let Self { head, tail } = self;
        let pack = if let Some(pack) = head {
            Some(device.allocate_material_pack_memory(allocator, pack)?)
        } else {
            None
        };
        Ok(Cons {
            head: pack,
            tail: tail.allocate(device, allocator)?,
        })
    }
}

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
    <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
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

impl VulkanDevice {
    pub fn destroy_materials<A: Allocator, M: MaterialPackList<A>>(
        &self,
        packs: &mut M,
        allocator: &mut A,
    ) {
        packs.destroy(self, allocator)
    }
}

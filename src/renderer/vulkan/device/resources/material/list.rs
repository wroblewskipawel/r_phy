use std::error::Error;

use crate::{
    core::{Cons, Nil},
    renderer::{
        model::{MaterialCollection, MaterialTypeList},
        vulkan::device::VulkanDevice,
    },
};

use super::{MaterialPack, MaterialPackRef, VulkanMaterial};

pub trait MaterialPackList: MaterialTypeList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>>;
}

impl MaterialPackList for Nil {
    fn destroy(&mut self, _device: &VulkanDevice) {}
    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>> {
        None
    }
}

impl<M: VulkanMaterial, N: MaterialPackList> MaterialTypeList for Cons<Option<MaterialPack<M>>, N> {
    const LEN: usize = N::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<M: VulkanMaterial, N: MaterialPackList> MaterialPackList for Cons<Option<MaterialPack<M>>, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        if let Some(material_pack) = &mut self.head {
            device.destroy_material_pack(material_pack);
        }
        self.tail.destroy(device);
    }

    fn try_get<T: VulkanMaterial>(&self) -> Option<MaterialPackRef<T>> {
        self.head
            .as_ref()
            .and_then(|pack| pack.try_into().ok())
            .or_else(|| self.tail.try_get::<T>())
    }
}

pub trait MaterialPackListBuilder: MaterialTypeList + 'static {
    type Pack: MaterialPackList;
    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl<M: VulkanMaterial, N: MaterialPackListBuilder> MaterialPackListBuilder for Cons<Vec<M>, N> {
    type Pack = Cons<Option<MaterialPack<Self::Item>>, N::Pack>;
    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        let materials = self.get();
        Ok(Cons {
            head: if !materials.is_empty() {
                Some(device.load_material_pack(materials)?)
            } else {
                None
            },
            tail: self.next().build(device)?,
        })
    }
}

impl MaterialPackListBuilder for Nil {
    type Pack = Self;

    fn build(&self, _device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Nil {})
    }
}

pub struct MaterialPacks<N: MaterialPackList> {
    pub packs: N,
}

impl VulkanDevice {
    pub fn load_materials<B: MaterialPackListBuilder>(
        &mut self,
        material_types: &B,
    ) -> Result<MaterialPacks<B::Pack>, Box<dyn Error>> {
        Ok(MaterialPacks {
            packs: material_types.build(self)?,
        })
    }

    pub fn destroy_materials<M: MaterialPackList>(&self, packs: &mut MaterialPacks<M>) {
        packs.packs.destroy(self)
    }
}

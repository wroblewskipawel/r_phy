use std::{error::Error, marker::PhantomData};

use crate::{
    core::{Cons, Nil},
    renderer::{
        model::{MaterialCollection, MaterialHandle, MaterialTypeList},
        vulkan::device::VulkanDevice,
    },
};

use super::{MaterialPack, MaterialPackData, MaterialPackRef, VulkanMaterial};

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
                Some(device.load_material_pack(materials, Self::LEN)?)
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

pub struct VulkanMaterialHandle<M: VulkanMaterial> {
    pub material_pack_index: u32,
    pub material_index: u32,
    _phantom: PhantomData<M>,
}

impl<M: VulkanMaterial> VulkanMaterialHandle<M> {
    pub fn new(material_pack_index: u32, material_index: u32) -> Self {
        Self {
            material_pack_index,
            material_index,
            _phantom: PhantomData,
        }
    }
}

impl<M: VulkanMaterial> From<MaterialHandle<M>> for VulkanMaterialHandle<M> {
    fn from(value: MaterialHandle<M>) -> Self {
        Self {
            material_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            material_index: (0x00000000FFFFFFFF & value.0) as u32,
            _phantom: PhantomData,
        }
    }
}

impl<M: VulkanMaterial> From<VulkanMaterialHandle<M>> for MaterialHandle<M> {
    fn from(value: VulkanMaterialHandle<M>) -> Self {
        Self(
            ((value.material_pack_index as u64) << 32) + value.material_index as u64,
            PhantomData,
        )
    }
}

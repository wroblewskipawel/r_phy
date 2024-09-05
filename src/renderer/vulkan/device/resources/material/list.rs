use std::{error::Error, marker::PhantomData};

use crate::renderer::{
    model::{
        MaterialCollection, MaterialHandle, MaterialTypeList, MaterialTypeNode,
        MaterialTypeTerminator,
    },
    vulkan::device::VulkanDevice,
};

use super::{MaterialPackRef, MaterialPackTypeErased, VulkanMaterial};

pub trait MaterialPackList: MaterialTypeList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>>;
}

impl MaterialPackList for MaterialTypeTerminator {
    fn destroy(&mut self, _device: &VulkanDevice) {}
    fn try_get<M: VulkanMaterial>(&self) -> Option<MaterialPackRef<M>> {
        None
    }
}

pub struct MaterialPackNode<M: VulkanMaterial, N: MaterialPackList> {
    pack: MaterialPackTypeErased,
    next: N,
    _phantom: PhantomData<M>,
}

impl<M: VulkanMaterial, N: MaterialPackList> MaterialTypeList for MaterialPackNode<M, N> {
    const LEN: usize = N::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<M: VulkanMaterial, N: MaterialPackList> MaterialPackList for MaterialPackNode<M, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        device.destroy_material_pack(&mut self.pack);
        self.next.destroy(device);
    }

    fn try_get<T: VulkanMaterial>(&self) -> Option<MaterialPackRef<T>> {
        if let Ok(material_pack_ref) = (&self.pack).try_into() {
            Some(material_pack_ref)
        } else {
            self.next.try_get::<T>()
        }
    }
}

pub trait MaterialPackListBuilder: MaterialTypeList + 'static {
    type Pack: MaterialPackList;
    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl<M: VulkanMaterial, N: MaterialPackListBuilder> MaterialPackListBuilder
    for MaterialTypeNode<M, N>
{
    type Pack = MaterialPackNode<Self::Item, N::Pack>;
    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(MaterialPackNode {
            pack: device.load_material_pack(self.get(), Self::LEN)?.into(),
            next: self.next().build(device)?,
            _phantom: PhantomData,
        })
    }
}

impl MaterialPackListBuilder for MaterialTypeTerminator {
    type Pack = Self;

    fn build(&self, _device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(MaterialTypeTerminator {})
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

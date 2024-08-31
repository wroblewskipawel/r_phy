use std::{any::TypeId, marker::PhantomData};

use ash::vk;

use crate::renderer::{
    model::MaterialHandle,
    vulkan::device::{
        buffer::{PersistentBuffer, UniformBufferTypeErased},
        command::operation::Graphics,
        descriptor::{Descriptor, DescriptorPoolRef, DescriptorPoolTypeErased},
        image::Texture2D,
    },
};

use super::{MaterialPack, MaterialPackData, VulkanMaterial, VulkanMaterialHandle};

pub struct MaterialPackTypeErased {
    type_id: TypeId,
    index: usize,
    textures: Option<Vec<Texture2D>>,
    uniforms: Option<UniformBufferTypeErased<Graphics>>,
    descriptors: DescriptorPoolTypeErased,
}

impl<M: VulkanMaterial> From<MaterialPack<M>> for MaterialPackTypeErased {
    fn from(value: MaterialPack<M>) -> Self {
        Self {
            type_id: TypeId::of::<M>(),
            index: value.index,
            textures: value.textures,
            uniforms: value.uniforms.map(|uniforms| uniforms.into()),
            descriptors: value.descriptors.into(),
        }
    }
}

impl MaterialPackData for MaterialPackTypeErased {
    fn get_textures(&mut self) -> Option<&mut Vec<Texture2D>> {
        self.textures.as_mut()
    }

    fn get_uniforms(&mut self) -> Option<&mut PersistentBuffer> {
        self.uniforms.as_mut().map(|uniform| uniform.into())
    }

    fn get_descriptor_pool(&mut self) -> vk::DescriptorPool {
        (&mut self.descriptors).into()
    }
}

pub struct MaterialPackRef<'a, M: VulkanMaterial> {
    index: usize,
    descriptors: DescriptorPoolRef<'a, M::DescriptorLayout>,
    _phantom: PhantomData<M>,
}

impl<'a, M: VulkanMaterial> TryFrom<&'a MaterialPackTypeErased> for MaterialPackRef<'a, M> {
    type Error = &'static str;

    fn try_from(value: &'a MaterialPackTypeErased) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<M>() {
            Ok(Self {
                index: value.index,
                descriptors: (&value.descriptors).try_into().unwrap(),
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid Material type")
        }
    }
}

impl<'a, M: VulkanMaterial> MaterialPackRef<'a, M> {
    pub fn get_descriptor(&self, index: usize) -> Descriptor<M::DescriptorLayout> {
        self.descriptors.get(index)
    }

    pub fn get_handles(&self) -> Vec<MaterialHandle<M>> {
        (0..self.descriptors.len())
            .map(|material_index| {
                VulkanMaterialHandle::new(self.index as u32, material_index as u32).into()
            })
            .collect()
    }
}

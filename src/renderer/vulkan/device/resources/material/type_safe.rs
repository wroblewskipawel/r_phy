use std::{error::Error, ops::Index};

use ash::vk;

use crate::renderer::vulkan::device::{
    descriptor::{Descriptor, DescriptorPool, DescriptorSetWriter},
    image::Texture2D,
    VulkanDevice,
};

use super::{MaterialPackData, VulkanMaterial};

pub struct MaterialPack<M: VulkanMaterial> {
    pub index: usize,
    pub textures: Vec<Texture2D>,
    pub descriptors: DescriptorPool<M::DescriptorLayout>,
}

impl<M: VulkanMaterial> Index<usize> for MaterialPack<M> {
    type Output = Descriptor<M::DescriptorLayout>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.descriptors[index]
    }
}

impl<M: VulkanMaterial> MaterialPackData for MaterialPack<M> {
    fn get_textures(&mut self) -> &mut Vec<Texture2D> {
        &mut self.textures
    }

    fn get_descriptor_pool(&mut self) -> vk::DescriptorPool {
        (&mut self.descriptors).into()
    }
}

impl VulkanDevice {
    pub fn load_material_pack<M: VulkanMaterial>(
        &self,
        materials: &[M],
        index: usize,
    ) -> Result<MaterialPack<M>, Box<dyn Error>> {
        let textures = materials
            .iter()
            .flat_map(|material| material.images().map(|image| self.load_texture(image)))
            .collect::<Result<Vec<_>, _>>()?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<M::DescriptorLayout>::new(materials.len())
                .write_images::<M, _>(&textures),
        )?;
        Ok(MaterialPack {
            index,
            textures,
            descriptors,
        })
    }
}

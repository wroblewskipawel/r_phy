use crate::renderer::model::Material;
use ash::vk;
use std::error::Error;

use super::{descriptor::DescriptorPool, image::Texture2D, VulkanDevice};

pub struct MaterialPack {
    textures: Vec<Texture2D>,
    pub descriptors: DescriptorPool<Texture2D>,
}

impl VulkanDevice {
    pub fn load_material_pack(
        &self,
        materials: &[Material],
    ) -> Result<MaterialPack, Box<dyn Error>> {
        let textures = materials
            .iter()
            .map(|material| self.load_texture(material.albedo))
            .collect::<Result<Vec<_>, _>>()?;
        let mut descriptors = self
            .create_descriptor_pool(textures.len(), vk::DescriptorType::COMBINED_IMAGE_SAMPLER)?;
        self.write_image_samplers(&mut descriptors, &textures);
        Ok(MaterialPack {
            textures,
            descriptors,
        })
    }

    pub fn destory_material_pack(&self, pack: &mut MaterialPack) {
        self.destory_descriptor_pool(&mut pack.descriptors);
        pack.textures
            .iter_mut()
            .for_each(|texture| self.destory_texture(texture));
    }
}

use crate::renderer::model::Material;
use ash::vk;
use std::error::Error;

use super::{
    descriptor::{DescriptorPool, TextureDescriptorSet},
    image::Texture2D,
    VulkanDevice,
};

pub struct MaterialPack {
    textures: Vec<Texture2D>,
    pub descriptors: DescriptorPool<TextureDescriptorSet>,
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
        let mut descriptors =
            self.create_descriptor_pool(TextureDescriptorSet::builder(), textures.len())?;
        let descriptor_write = descriptors.get_writer().write_image(&textures);
        self.write_descriptor_sets(&mut descriptors, descriptor_write);
        Ok(MaterialPack {
            textures,
            descriptors,
        })
    }

    pub fn destroy_material_pack(&self, pack: &mut MaterialPack) {
        self.destroy_descriptor_pool(&mut pack.descriptors);
        pack.textures
            .iter_mut()
            .for_each(|texture| self.destroy_texture(texture));
    }
}

use std::{error::Error, ops::Index};

use ash::vk;

use crate::renderer::vulkan::device::{
    buffer::{PersistentBuffer, UniformBuffer},
    command::operation::Graphics,
    descriptor::{Descriptor, DescriptorPool, DescriptorSetWriter, FragmentStage, PodUniform},
    image::Texture2D,
    VulkanDevice,
};

use super::{MaterialPackData, TextureSamplers, VulkanMaterial};

pub struct MaterialPack<M: VulkanMaterial> {
    pub index: usize,
    pub textures: Option<Vec<Texture2D>>,
    pub uniforms: Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics>>,
    pub descriptors: DescriptorPool<M::DescriptorLayout>,
}

impl<M: VulkanMaterial> Index<usize> for MaterialPack<M> {
    type Output = Descriptor<M::DescriptorLayout>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.descriptors[index]
    }
}

impl<M: VulkanMaterial> MaterialPackData for MaterialPack<M> {
    fn get_textures(&mut self) -> Option<&mut Vec<Texture2D>> {
        self.textures.as_mut()
    }

    fn get_uniforms(&mut self) -> Option<&mut PersistentBuffer> {
        self.uniforms.as_mut().map(|uniforms| uniforms.into())
    }

    fn get_descriptor_pool(&mut self) -> vk::DescriptorPool {
        (&mut self.descriptors).into()
    }
}

impl VulkanDevice {
    fn load_material_pack_textures<M: VulkanMaterial>(
        &mut self,
        materials: &[M],
    ) -> Result<Option<Vec<Texture2D>>, Box<dyn Error>> {
        if M::NUM_IMAGES > 0 {
            let textures = materials
                .iter()
                .flat_map(|material| {
                    // TODO: It would be better to create vector of iterators and flatten them
                    // Currently unable to do this because of the lifetime of the iterator
                    material
                        .images()
                        .unwrap()
                        .map(|image| self.load_texture(image))
                        .collect::<Vec<_>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(textures))
        } else {
            Ok(None)
        }
    }

    fn load_material_pack_uniforms<M: VulkanMaterial>(
        &mut self,
        materials: &[M],
    ) -> Result<
        Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics>>,
        Box<dyn Error>,
    > {
        if let Some(uniform_data) = materials
            .iter()
            .map(|material| material.uniform())
            .collect::<Option<Vec<_>>>()
        {
            let mut uniform_buffer = self
                .create_uniform_buffer::<PodUniform<M::Uniform, FragmentStage>, Graphics>(
                    materials.len(),
                )?;
            for (index, uniform) in uniform_data.into_iter().enumerate() {
                *uniform_buffer[index].as_inner_mut() = *uniform;
            }
            Ok(Some(uniform_buffer))
        } else {
            Ok(None)
        }
    }

    pub fn load_material_pack<M: VulkanMaterial>(
        &mut self,
        materials: &[M],
        index: usize,
    ) -> Result<MaterialPack<M>, Box<dyn Error>> {
        let textures = self.load_material_pack_textures(materials)?;
        let uniforms = self.load_material_pack_uniforms(materials)?;
        let writer = DescriptorSetWriter::<M::DescriptorLayout>::new(materials.len());
        let writer = if let Some(textures) = &textures {
            writer.write_images::<TextureSamplers<M>, _>(textures)
        } else {
            writer
        };
        let writer = if let Some(uniforms) = &uniforms {
            writer.write_buffer(uniforms)
        } else {
            writer
        };
        let descriptors = self.create_descriptor_pool(writer)?;
        Ok(MaterialPack {
            index,
            textures,
            uniforms,
            descriptors,
        })
    }
}

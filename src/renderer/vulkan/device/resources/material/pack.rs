use std::{any::TypeId, error::Error, marker::PhantomData};

use crate::renderer::vulkan::device::{
    buffer::UniformBuffer,
    command::operation::Graphics,
    descriptor::{
        Descriptor, DescriptorPool, DescriptorPoolRef, DescriptorSetWriter, FragmentStage,
        PodUniform,
    },
    image::Texture2D,
    VulkanDevice,
};

use super::{TextureSamplers, VulkanMaterial};

pub struct MaterialPackData<M: VulkanMaterial> {
    textures: Option<Vec<Texture2D>>,
    uniforms: Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics>>,
    descriptors: DescriptorPool<M::DescriptorLayout>,
}

pub struct MaterialPack<M: VulkanMaterial> {
    data: MaterialPackData<M>,
}

impl<'a, M: VulkanMaterial> From<&'a MaterialPack<M>> for &'a MaterialPackData<M> {
    fn from(pack: &'a MaterialPack<M>) -> Self {
        &pack.data
    }
}

impl<'a, M: VulkanMaterial> From<&'a mut MaterialPack<M>> for &'a mut MaterialPackData<M> {
    fn from(pack: &'a mut MaterialPack<M>) -> Self {
        &mut pack.data
    }
}

pub struct MaterialPackRef<'a, M: VulkanMaterial> {
    descriptors: DescriptorPoolRef<'a, M::DescriptorLayout>,
    _phantom: PhantomData<M>,
}

impl<'a, M: VulkanMaterial, T: VulkanMaterial> TryFrom<&'a MaterialPack<M>>
    for MaterialPackRef<'a, T>
{
    type Error = &'static str;

    fn try_from(value: &'a MaterialPack<M>) -> Result<Self, Self::Error> {
        if TypeId::of::<M>() == TypeId::of::<T>() {
            Ok(Self {
                descriptors: (&value.data.descriptors).try_into().unwrap(),
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
        let uniform_data = materials
            .iter()
            .filter_map(|material| material.uniform())
            .collect::<Vec<_>>();
        if !uniform_data.is_empty() {
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
        let data = MaterialPackData {
            textures,
            uniforms,
            descriptors,
        };
        Ok(MaterialPack { data })
    }
}

impl VulkanDevice {
    pub fn destroy_material_pack<'a, M: VulkanMaterial>(
        &self,
        pack: impl Into<&'a mut MaterialPackData<M>>,
    ) {
        let data = pack.into();
        if let Some(textures) = data.textures.as_mut() {
            textures
                .iter_mut()
                .for_each(|texture| self.destroy_texture(texture));
        }
        if let Some(uniforms) = data.uniforms.as_mut() {
            self.destroy_uniform_buffer(uniforms);
        }
        self.destroy_descriptor_pool(&mut data.descriptors);
    }
}

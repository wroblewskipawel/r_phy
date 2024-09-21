use std::{any::TypeId, error::Error, marker::PhantomData};

use crate::renderer::vulkan::device::{
    buffer::UniformBuffer,
    command::operation::Graphics,
    descriptor::{
        Descriptor, DescriptorPool, DescriptorPoolRef, DescriptorSetWriter, FragmentStage,
        PodUniform,
    },
    image::Texture2D,
    memory::{Allocator, HostCoherent, HostVisibleMemory},
    VulkanDevice,
};

use super::{TextureSamplers, VulkanMaterial};

pub struct MaterialPackData<M: VulkanMaterial, A: Allocator> {
    textures: Option<Vec<Texture2D<A>>>,
    uniforms: Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>>,
    descriptors: DescriptorPool<M::DescriptorLayout>,
}

pub struct MaterialPack<M: VulkanMaterial, A: Allocator> {
    data: MaterialPackData<M, A>,
}

impl<'a, M: VulkanMaterial, A: Allocator> From<&'a MaterialPack<M, A>>
    for &'a MaterialPackData<M, A>
{
    fn from(pack: &'a MaterialPack<M, A>) -> Self {
        &pack.data
    }
}

impl<'a, M: VulkanMaterial, A: Allocator> From<&'a mut MaterialPack<M, A>>
    for &'a mut MaterialPackData<M, A>
{
    fn from(pack: &'a mut MaterialPack<M, A>) -> Self {
        &mut pack.data
    }
}

pub struct MaterialPackRef<'a, M: VulkanMaterial> {
    descriptors: DescriptorPoolRef<'a, M::DescriptorLayout>,
    _phantom: PhantomData<M>,
}

impl<'a, A: Allocator, M: VulkanMaterial, T: VulkanMaterial> TryFrom<&'a MaterialPack<M, A>>
    for MaterialPackRef<'a, T>
{
    type Error = &'static str;

    fn try_from(value: &'a MaterialPack<M, A>) -> Result<Self, Self::Error> {
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
    fn load_material_pack_textures<M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        materials: &[M],
    ) -> Result<Option<Vec<Texture2D<A>>>, Box<dyn Error>> {
        if M::NUM_IMAGES > 0 {
            let textures = materials
                .iter()
                .flat_map(|material| {
                    // TODO: It would be better to create vector of iterators and flatten them
                    // Currently unable to do this because of the lifetime of the iterator
                    material
                        .images()
                        .unwrap()
                        .map(|image| self.load_texture(allocator, image))
                        .collect::<Vec<_>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(textures))
        } else {
            Ok(None)
        }
    }

    fn load_material_pack_uniforms<M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        materials: &[M],
    ) -> Result<
        Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>>,
        Box<dyn Error>,
    >
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let uniform_data = materials
            .iter()
            .filter_map(|material| material.uniform())
            .collect::<Vec<_>>();
        if !uniform_data.is_empty() {
            let mut uniform_buffer = self
                .create_uniform_buffer::<PodUniform<M::Uniform, FragmentStage>, Graphics, _>(
                    allocator,
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

    pub fn load_material_pack<M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        materials: &[M],
    ) -> Result<MaterialPack<M, A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let textures = self.load_material_pack_textures(allocator, materials)?;
        let uniforms = self.load_material_pack_uniforms(allocator, materials)?;
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
    pub fn destroy_material_pack<'a, M: VulkanMaterial, A: Allocator>(
        &self,
        pack: impl Into<&'a mut MaterialPackData<M, A>>,
        allocator: &mut A,
    ) where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let data = pack.into();
        if let Some(textures) = data.textures.as_mut() {
            textures
                .iter_mut()
                .for_each(|texture| self.destroy_texture(texture, allocator));
        }
        if let Some(uniforms) = data.uniforms.as_mut() {
            self.destroy_uniform_buffer(uniforms, allocator);
        }
        self.destroy_descriptor_pool(&mut data.descriptors);
    }
}

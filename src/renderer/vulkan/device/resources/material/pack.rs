use std::{any::TypeId, error::Error, marker::PhantomData};

use crate::renderer::vulkan::device::{
    command::operation::Graphics,
    descriptor::{
        Descriptor, DescriptorPool, DescriptorPoolRef, DescriptorSetWriter, FragmentStage,
        PodUniform,
    },
    memory::{AllocReq, AllocReqRaw, Allocator, HostCoherent, HostVisibleMemory},
    resources::{
        buffer::{UniformBuffer, UniformBufferBuilder, UniformBufferPartial},
        image::{Texture2D, Texture2DPartial},
        FromPartial, Partial, PartialBuilder,
    },
    VulkanDevice,
};

use super::{TextureSamplers, VulkanMaterial};

struct MaterialUniformPartial<'a, M: VulkanMaterial> {
    uniform: UniformBufferPartial<PodUniform<M::Uniform, FragmentStage>, Graphics>,
    data: Vec<&'a M::Uniform>,
}

pub struct MaterialPackData<M: VulkanMaterial, A: Allocator> {
    textures: Option<Vec<Texture2D<A>>>,
    uniforms: Option<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>>,
    descriptors: DescriptorPool<M::DescriptorLayout>,
}

pub struct MaterialPackPartial<'a, M: VulkanMaterial> {
    textures: Option<Vec<Texture2DPartial<'a>>>,
    uniforms: Option<MaterialUniformPartial<'a, M>>,
    num_materials: usize,
}

impl<'a, M: VulkanMaterial> MaterialPackPartial<'a, M> {
    pub fn get_alloc_req_raw(&self) -> impl Iterator<Item = AllocReqRaw> {
        let mut alloc_reqs: Vec<AllocReqRaw> = if let Some(buffer) = &self.uniforms {
            vec![buffer.uniform.requirements().into()]
        } else {
            vec![]
        };
        if let Some(textures) = &self.textures {
            alloc_reqs.extend(
                textures.iter().map(|texture| {
                    <AllocReq<_> as Into<AllocReqRaw>>::into(texture.get_alloc_req())
                }),
            );
        }
        alloc_reqs.into_iter()
    }
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
    fn prepare_material_pack_textures<'a, M: VulkanMaterial>(
        &self,
        materials: &'a [M],
    ) -> Result<Option<Vec<Texture2DPartial<'a>>>, Box<dyn Error>> {
        if M::NUM_IMAGES > 0 {
            let textures = materials
                .iter()
                .flat_map(|material| {
                    // TODO: It would be better to create vector of iterators and flatten them
                    // Currently unable to do this because of the lifetime of the iterator
                    material
                        .images()
                        .unwrap()
                        .map(|image| image.prepare(self))
                        .collect::<Vec<_>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(textures))
        } else {
            Ok(None)
        }
    }

    fn allocate_material_pack_textures_memory<'a, A: Allocator>(
        &self,
        allocator: &mut A,
        textures: Vec<Texture2DPartial<'a>>,
    ) -> Result<Vec<Texture2D<A>>, Box<dyn Error>> {
        textures
            .into_iter()
            .map(|texture| Texture2D::finalize(texture, self, allocator))
            .collect()
    }

    fn prepare_material_pack_uniforms<'a, M: VulkanMaterial>(
        &self,
        materials: &'a [M],
    ) -> Result<Option<MaterialUniformPartial<'a, M>>, Box<dyn Error>> {
        let data = materials
            .iter()
            .filter_map(|material| material.uniform())
            .collect::<Vec<_>>();
        if !data.is_empty() {
            let uniform = UniformBufferBuilder::new(materials.len()).prepare(self)?;
            Ok(Some(MaterialUniformPartial { uniform, data }))
        } else {
            Ok(None)
        }
    }

    fn allocate_material_pack_uniforms_memory<'a, M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        partial: MaterialUniformPartial<'a, M>,
    ) -> Result<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let MaterialUniformPartial { uniform, data } = partial;
        let mut uniform_buffer = UniformBuffer::finalize(uniform, self, allocator)?;
        for (index, uniform) in data.into_iter().enumerate() {
            *uniform_buffer[index].as_inner_mut() = *uniform;
        }
        Ok(uniform_buffer)
    }

    pub fn prepare_material_pack<'a, M: VulkanMaterial>(
        &self,
        materials: &'a [M],
    ) -> Result<MaterialPackPartial<'a, M>, Box<dyn Error>> {
        let textures = self.prepare_material_pack_textures(materials)?;
        let uniforms = self.prepare_material_pack_uniforms(materials)?;
        Ok(MaterialPackPartial {
            textures,
            uniforms,
            num_materials: materials.len(),
        })
    }

    pub fn allocate_material_pack_memory<'a, M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        partial: MaterialPackPartial<'a, M>,
    ) -> Result<MaterialPack<M, A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let MaterialPackPartial {
            textures,
            uniforms,
            num_materials,
        } = partial;
        let textures = if let Some(textures) = textures {
            Some(self.allocate_material_pack_textures_memory(allocator, textures)?)
        } else {
            None
        };
        let uniforms = if let Some(uniforms) = uniforms {
            Some(self.allocate_material_pack_uniforms_memory(allocator, uniforms)?)
        } else {
            None
        };
        let writer = DescriptorSetWriter::<M::DescriptorLayout>::new(num_materials);
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

    pub fn load_material_pack<M: VulkanMaterial, A: Allocator>(
        &self,
        allocator: &mut A,
        materials: &[M],
    ) -> Result<MaterialPack<M, A>, Box<dyn Error>>
    where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let pack = self.prepare_material_pack(materials)?;
        let pack = self.allocate_material_pack_memory(allocator, pack)?;
        Ok(pack)
    }
}

impl VulkanDevice {
    pub fn destroy_material_pack<'a, M: VulkanMaterial, A: Allocator>(
        &self,
        pack: impl Into<&'a mut MaterialPackData<M, A>>,
        allocator: &mut A,
    ) where
        <A as Allocator>::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let data = pack.into();
        if let Some(textures) = data.textures.as_mut() {
            textures
                .iter_mut()
                .for_each(|texture| self.destroy_texture(texture, allocator));
        }
        if let Some(uniforms) = data.uniforms.as_mut() {
            self.destroy_buffer(uniforms, allocator);
        }
        self.destroy_descriptor_pool(&mut data.descriptors);
    }
}

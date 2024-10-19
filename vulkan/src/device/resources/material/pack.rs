use std::{any::TypeId, error::Error, marker::PhantomData};

use type_kit::{Destroy, DropGuard};

use crate::device::{
    command::operation::Graphics,
    descriptor::{
        Descriptor, DescriptorPool, DescriptorPoolRef, DescriptorSetWriter, FragmentStage,
        PodUniform,
    },
    memory::{AllocReq, Allocator},
    resources::{
        buffer::{UniformBuffer, UniformBufferBuilder, UniformBufferPartial},
        image::{ImageReader, Texture2D, Texture2DPartial},
        PartialBuilder,
    },
    Device,
};

use super::{Material, TextureSamplers};

struct MaterialUniformPartial<'a, M: Material> {
    uniform: UniformBufferPartial<PodUniform<M::Uniform, FragmentStage>, Graphics>,
    data: Vec<&'a M::Uniform>,
}

pub struct MaterialPackData<M: Material, A: Allocator> {
    textures: Option<Vec<Texture2D<A>>>,
    uniforms: Option<DropGuard<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>>>,
    descriptors: DropGuard<DescriptorPool<M::DescriptorLayout>>,
}

pub struct MaterialPackPartial<'a, M: Material> {
    textures: Option<Vec<Texture2DPartial<'a>>>,
    uniforms: Option<MaterialUniformPartial<'a, M>>,
    num_materials: usize,
}

impl<'a, M: Material> MaterialPackPartial<'a, M> {
    pub fn get_alloc_req(&self) -> impl Iterator<Item = AllocReq> {
        let mut alloc_reqs: Vec<AllocReq> = if let Some(buffer) = &self.uniforms {
            buffer.uniform.requirements().collect()
        } else {
            vec![]
        };
        if let Some(textures) = &self.textures {
            alloc_reqs.extend(
                textures
                    .iter()
                    .map(|texture| texture.requirements())
                    .flatten(),
            );
        }
        alloc_reqs.into_iter()
    }
}

pub struct MaterialPack<M: Material, A: Allocator> {
    data: MaterialPackData<M, A>,
}

impl<'a, M: Material, A: Allocator> From<&'a MaterialPack<M, A>> for &'a MaterialPackData<M, A> {
    fn from(pack: &'a MaterialPack<M, A>) -> Self {
        &pack.data
    }
}

impl<'a, M: Material, A: Allocator> From<&'a mut MaterialPack<M, A>>
    for &'a mut MaterialPackData<M, A>
{
    fn from(pack: &'a mut MaterialPack<M, A>) -> Self {
        &mut pack.data
    }
}

pub struct MaterialPackRef<'a, M: Material> {
    descriptors: DescriptorPoolRef<'a, M::DescriptorLayout>,
    _phantom: PhantomData<M>,
}

impl<'a, A: Allocator, M: Material, T: Material> TryFrom<&'a MaterialPack<M, A>>
    for MaterialPackRef<'a, T>
{
    type Error = &'static str;

    fn try_from(value: &'a MaterialPack<M, A>) -> Result<Self, Self::Error> {
        if TypeId::of::<M>() == TypeId::of::<T>() {
            Ok(Self {
                descriptors: (&*value.data.descriptors).try_into().unwrap(),
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid Material type")
        }
    }
}

impl<'a, M: Material> MaterialPackRef<'a, M> {
    pub fn get_descriptor(&self, index: usize) -> Descriptor<M::DescriptorLayout> {
        self.descriptors.get(index)
    }
}

impl Device {
    fn prepare_material_pack_textures<'a, M: Material>(
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
                        .map(|image| Texture2DPartial::prepare(ImageReader::image(image)?, self))
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
            .map(|texture| texture.finalize(self, allocator))
            .collect()
    }

    fn prepare_material_pack_uniforms<'a, M: Material>(
        &self,
        materials: &'a [M],
    ) -> Result<Option<MaterialUniformPartial<'a, M>>, Box<dyn Error>> {
        let data = materials
            .iter()
            .filter_map(|material| material.uniform())
            .collect::<Vec<_>>();
        if !data.is_empty() {
            let uniform =
                UniformBufferPartial::prepare(UniformBufferBuilder::new(materials.len()), self)?;
            Ok(Some(MaterialUniformPartial { uniform, data }))
        } else {
            Ok(None)
        }
    }

    fn allocate_material_pack_uniforms_memory<'a, M: Material, A: Allocator>(
        &self,
        allocator: &mut A,
        partial: MaterialUniformPartial<'a, M>,
    ) -> Result<UniformBuffer<PodUniform<M::Uniform, FragmentStage>, Graphics, A>, Box<dyn Error>>
    {
        let MaterialUniformPartial { uniform, data } = partial;
        let mut uniform_buffer = uniform.finalize(self, allocator)?;
        for (index, uniform) in data.into_iter().enumerate() {
            *uniform_buffer[index].as_inner_mut() = *uniform;
        }
        Ok(uniform_buffer)
    }

    pub fn prepare_material_pack<'a, M: Material>(
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

    pub fn allocate_material_pack_memory<'a, M: Material, A: Allocator>(
        &self,
        allocator: &mut A,
        partial: MaterialPackPartial<'a, M>,
    ) -> Result<MaterialPack<M, A>, Box<dyn Error>> {
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
            Some(DropGuard::new(
                self.allocate_material_pack_uniforms_memory(allocator, uniforms)?,
            ))
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
            descriptors: DropGuard::new(descriptors),
        };
        Ok(MaterialPack { data })
    }

    pub fn load_material_pack<M: Material, A: Allocator>(
        &self,
        allocator: &mut A,
        materials: &[M],
    ) -> Result<MaterialPack<M, A>, Box<dyn Error>> {
        let pack = self.prepare_material_pack(materials)?;
        let pack = self.allocate_material_pack_memory(allocator, pack)?;
        Ok(pack)
    }
}

impl Device {
    pub fn destroy_material_pack<'a, M: Material, A: Allocator>(
        &self,
        pack: impl Into<&'a mut MaterialPackData<M, A>>,
        allocator: &mut A,
    ) {
        let data = pack.into();
        if let Some(textures) = data.textures.as_mut() {
            textures
                .iter_mut()
                .for_each(|texture| texture.destroy((self, allocator)));
        }
        if let Some(uniforms) = data.uniforms.as_mut() {
            uniforms.destroy((self, allocator));
        }
        data.descriptors.destroy(self);
    }
}

use ash::vk;

use crate::renderer::{
    model::{
        Material, MaterialCollection, MaterialHandle, MaterialTypeList, MaterialTypeNode,
        MaterialTypeTerminator,
    },
    vulkan::device::descriptor::{
        Descriptor, DescriptorPool, DescriptorPoolRaw, DescriptorSetWriter,
    },
};
use std::{any::TypeId, error::Error, marker::PhantomData};

use crate::renderer::vulkan::device::{
    descriptor::{
        DescriptorBinding, DescriptorBindingNode, DescriptorBindingTerminator, DescriptorLayout,
        DescriptorLayoutBuilder,
    },
    image::Texture2D,
    VulkanDevice,
};

impl<T: Material> DescriptorBinding for T {
    fn get_descriptor_set_binding(binding: u32) -> ash::vk::DescriptorSetLayoutBinding {
        vk::DescriptorSetLayoutBinding {
            binding,
            descriptor_type: ash::vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: T::NUM_IMAGES as u32,
            stage_flags: ash::vk::ShaderStageFlags::FRAGMENT,
            p_immutable_samplers: std::ptr::null(),
        }
    }

    fn get_descriptor_write(binding: u32) -> ash::vk::WriteDescriptorSet {
        ash::vk::WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: T::NUM_IMAGES as u32,
            descriptor_type: ash::vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            ..Default::default()
        }
    }

    fn get_descriptor_pool_size(num_sets: u32) -> ash::vk::DescriptorPoolSize {
        ash::vk::DescriptorPoolSize {
            ty: ash::vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: num_sets * T::NUM_IMAGES as u32,
        }
    }
}

pub trait VulkanMaterial: Material {
    type DescriptorLayout: DescriptorLayout;
}

impl<T: Material + DescriptorBinding> VulkanMaterial for T {
    type DescriptorLayout =
        DescriptorLayoutBuilder<DescriptorBindingNode<T, DescriptorBindingTerminator>>;
}

pub struct MaterialPackRaw {
    index: usize,
    textures: Vec<Texture2D>,
    pub descriptors: DescriptorPoolRaw,
}

pub struct MaterialPack<'a, M: VulkanMaterial> {
    index: usize,
    descriptors: DescriptorPool<'a, M::DescriptorLayout>,
    _phantom: PhantomData<M>,
}

impl<'a, M: Material> From<&'a MaterialPackRaw> for MaterialPack<'a, M> {
    fn from(value: &'a MaterialPackRaw) -> Self {
        Self {
            index: value.index,
            descriptors: (&value.descriptors).into(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, M: VulkanMaterial> MaterialPack<'a, M> {
    pub fn get_descriptor(&self, index: usize) -> Descriptor<M::DescriptorLayout> {
        self.descriptors.get(index)
    }

    pub fn get_handles(&self) -> Vec<MaterialHandle<M>> {
        (0..self.descriptors.raw.count)
            .map(|material_index| {
                VulkanMaterialHandle {
                    material_pack_index: self.index as u32,
                    material_index: material_index as u32,
                    _phantom: PhantomData,
                }
                .into()
            })
            .collect()
    }
}

impl VulkanDevice {
    pub fn load_material_pack<M: VulkanMaterial>(
        &self,
        materials: &[M],
        index: usize,
    ) -> Result<MaterialPackRaw, Box<dyn Error>> {
        let textures = materials
            .iter()
            .flat_map(|material| material.images().map(|image| self.load_texture(image)))
            .collect::<Result<Vec<_>, _>>()?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<M::DescriptorLayout>::new(materials.len())
                .write_images::<M, _>(&textures),
        )?;
        Ok(MaterialPackRaw {
            index,
            textures,
            descriptors,
        })
    }

    pub fn destroy_material_pack(&self, pack: &mut MaterialPackRaw) {
        self.destroy_descriptor_pool(&mut pack.descriptors);
        pack.textures
            .iter_mut()
            .for_each(|texture| self.destroy_texture(texture));
    }
}

pub trait MaterialPackList: MaterialTypeList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<'a, M: Material>(&'a self) -> Option<MaterialPack<'a, M>>;
}

impl MaterialPackList for MaterialTypeTerminator {
    fn destroy(&mut self, _device: &VulkanDevice) {}
    fn try_get<'a, M: Material>(&'a self) -> Option<MaterialPack<'a, M>> {
        None
    }
}

pub struct MaterialPackNode<M: Material, N: MaterialPackList> {
    pack: MaterialPackRaw,
    next: N,
    _phantom: PhantomData<M>,
}

impl<M: Material, N: MaterialPackList> MaterialTypeList for MaterialPackNode<M, N> {
    const LEN: usize = N::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<M: Material, N: MaterialPackList> MaterialPackList for MaterialPackNode<M, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        device.destroy_material_pack(&mut self.pack);
        self.next.destroy(device);
    }

    fn try_get<'a, T: Material>(&'a self) -> Option<MaterialPack<'a, T>> {
        if TypeId::of::<T>() == TypeId::of::<M>() {
            Some((&self.pack).into())
        } else {
            self.next.try_get::<T>()
        }
    }
}

pub trait MaterialPackListBuilder: MaterialTypeList + 'static {
    type Pack: MaterialPackList;
    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl<M: Material, N: MaterialPackListBuilder> MaterialPackListBuilder for MaterialTypeNode<M, N> {
    type Pack = MaterialPackNode<Self::Item, N::Pack>;
    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(MaterialPackNode {
            pack: device.load_material_pack(self.get(), Self::LEN)?,
            next: self.next().build(device)?,
            _phantom: PhantomData,
        })
    }
}

impl MaterialPackListBuilder for MaterialTypeTerminator {
    type Pack = Self;

    fn build(&self, _device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(MaterialTypeTerminator {})
    }
}

pub struct MaterialPacks<N: MaterialPackList> {
    pub packs: N,
}

impl VulkanDevice {
    pub fn load_materials<B: MaterialPackListBuilder>(
        &self,
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

pub struct VulkanMaterialHandle<M: Material> {
    pub material_pack_index: u32,
    pub material_index: u32,
    _phantom: PhantomData<M>,
}

impl<M: Material> From<MaterialHandle<M>> for VulkanMaterialHandle<M> {
    fn from(value: MaterialHandle<M>) -> Self {
        Self {
            material_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            material_index: (0x00000000FFFFFFFF & value.0) as u32,
            _phantom: PhantomData,
        }
    }
}

impl<M: Material> From<VulkanMaterialHandle<M>> for MaterialHandle<M> {
    fn from(value: VulkanMaterialHandle<M>) -> Self {
        Self(
            ((value.material_pack_index as u64) << 32) + value.material_index as u64,
            PhantomData,
        )
    }
}

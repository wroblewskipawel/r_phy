mod list;
mod type_erased;
mod type_safe;

use std::marker::PhantomData;

pub use list::*;
pub use type_erased::*;
pub use type_safe::*;

use ash::vk;

use crate::renderer::model::Material;

use crate::renderer::vulkan::device::buffer::PersistentBuffer;
use crate::renderer::vulkan::device::descriptor::{FragmentStage, PodUniform};
use crate::renderer::vulkan::device::image::Texture2D;
use crate::renderer::vulkan::device::{
    descriptor::{
        DescriptorBinding, DescriptorBindingNode, DescriptorBindingTerminator, DescriptorLayout,
        DescriptorLayoutBuilder,
    },
    VulkanDevice,
};

pub struct TextureSamplers<M: Material> {
    _phantom_data: PhantomData<M>,
}

impl<T: Material> DescriptorBinding for TextureSamplers<T> {
    fn has_data() -> bool {
        T::NUM_IMAGES > 0
    }

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

impl<T: Material> VulkanMaterial for T {
    type DescriptorLayout = DescriptorLayoutBuilder<
        DescriptorBindingNode<
            PodUniform<T::Uniform, FragmentStage>,
            DescriptorBindingNode<TextureSamplers<T>, DescriptorBindingTerminator>,
        >,
    >;
}

pub trait MaterialPackData {
    fn get_textures(&mut self) -> Option<&mut Vec<Texture2D>>;
    fn get_uniforms(&mut self) -> Option<&mut PersistentBuffer>;
    fn get_descriptor_pool(&mut self) -> vk::DescriptorPool;
}

impl VulkanDevice {
    pub fn destroy_material_pack(&self, pack: &mut impl MaterialPackData) {
        if let Some(textures) = pack.get_textures() {
            textures
                .iter_mut()
                .for_each(|texture| self.destroy_texture(texture));
        }
        if let Some(uniforms) = pack.get_uniforms() {
            self.destroy_persistent_buffer(uniforms);
        }
        self.destroy_descriptor_pool(pack.get_descriptor_pool());
    }
}

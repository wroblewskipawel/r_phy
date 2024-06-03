mod list;
mod type_erased;
mod type_safe;

pub use list::*;
pub use type_erased::*;
pub use type_safe::*;

use ash::vk;

use crate::renderer::model::Material;

use crate::renderer::vulkan::device::image::Texture2D;
use crate::renderer::vulkan::device::{
    descriptor::{
        DescriptorBinding, DescriptorBindingNode, DescriptorBindingTerminator, DescriptorLayout,
        DescriptorLayoutBuilder,
    },
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

pub trait MaterialPackData {
    fn get_textures(&mut self) -> &mut Vec<Texture2D>;
    fn get_descriptor_pool(&mut self) -> vk::DescriptorPool;
}

impl VulkanDevice {
    pub fn destroy_material_pack(&self, pack: &mut impl MaterialPackData) {
        pack.get_textures()
            .iter_mut()
            .for_each(|texture| self.destroy_texture(texture));
        self.destroy_descriptor_pool(pack.get_descriptor_pool());
    }
}

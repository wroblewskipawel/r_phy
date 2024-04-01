use ash::vk;

use crate::renderer::{camera::CameraMatrices, vulkan::device::image::Texture2D};

use super::{
    DescriptorBinding, DescriptorBindingNode, DescriptorBindingTerminator, DescriptorLayoutBuilder,
};

impl DescriptorBinding for CameraMatrices {
    fn get_descriptor_set_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
        vk::DescriptorSetLayoutBinding {
            binding: binding,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX,
            p_immutable_samplers: std::ptr::null(),
        }
    }

    fn get_descriptor_write(binding: u32) -> vk::WriteDescriptorSet {
        vk::WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            ..Default::default()
        }
    }

    fn get_descriptor_pool_size(num_sets: u32) -> vk::DescriptorPoolSize {
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1 * num_sets,
        }
    }
}

impl DescriptorBinding for Texture2D {
    fn get_descriptor_set_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
        vk::DescriptorSetLayoutBinding {
            binding: binding,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            p_immutable_samplers: std::ptr::null(),
        }
    }

    fn get_descriptor_write(binding: u32) -> vk::WriteDescriptorSet {
        vk::WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            ..Default::default()
        }
    }

    fn get_descriptor_pool_size(num_sets: u32) -> vk::DescriptorPoolSize {
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1 * num_sets,
        }
    }
}

pub type CameraDescriptorSet =
    DescriptorLayoutBuilder<DescriptorBindingNode<CameraMatrices, DescriptorBindingTerminator>>;
pub type TextureDescriptorSet =
    DescriptorLayoutBuilder<DescriptorBindingNode<Texture2D, DescriptorBindingTerminator>>;

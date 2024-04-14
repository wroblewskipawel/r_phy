use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::{
    math::types::Matrix4,
    renderer::{
        camera::CameraMatrices,
        vulkan::device::descriptor::{
            CameraDescriptorSet, GBufferDescriptorSet, TextureDescriptorSet,
        },
    },
};

use super::{
    DescriptorLayoutNode, DescriptorLayoutTerminator, PipelineLayoutBuilder, PushConstant,
    PushConstantNode, PushConstantTerminator,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct ModelMatrix(Matrix4);

impl From<Matrix4> for ModelMatrix {
    fn from(value: Matrix4) -> Self {
        ModelMatrix(value)
    }
}

impl PushConstant for ModelMatrix {
    fn range(offset: u32) -> ash::vk::PushConstantRange {
        vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset,
            size: size_of::<Self>() as u32,
        }
    }
}

impl PushConstant for CameraMatrices {
    fn range(offset: u32) -> vk::PushConstantRange {
        vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset,
            size: size_of::<Self>() as u32,
        }
    }
}

pub type PipelineLayoutTextured = PipelineLayoutBuilder<
    DescriptorLayoutNode<
        TextureDescriptorSet,
        DescriptorLayoutNode<CameraDescriptorSet, DescriptorLayoutTerminator>,
    >,
    PushConstantNode<ModelMatrix, PushConstantTerminator>,
>;

pub type PipelineLayoutSkybox = PipelineLayoutBuilder<
    DescriptorLayoutNode<TextureDescriptorSet, DescriptorLayoutTerminator>,
    PushConstantNode<CameraMatrices, PushConstantTerminator>,
>;

pub type PipelineLayoutNoMaterial = PipelineLayoutBuilder<
    DescriptorLayoutNode<CameraDescriptorSet, DescriptorLayoutTerminator>,
    PushConstantNode<ModelMatrix, PushConstantTerminator>,
>;

pub type PipelineLayoutGBuffer = PipelineLayoutBuilder<
    DescriptorLayoutNode<GBufferDescriptorSet, DescriptorLayoutTerminator>,
    PushConstantTerminator,
>;

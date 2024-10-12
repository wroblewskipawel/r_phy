use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::device::{
    descriptor::{CameraDescriptorSet, GBufferDescriptorSet, TextureDescriptorSet},
    resources::VulkanMaterial,
};
use math::types::{Matrix3, Matrix4};
use to_resolve::camera::CameraMatrices;
use type_list::{Cons, Nil};

use super::{PipelineLayoutBuilder, PushConstant};

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct ModelMatrix(Matrix4);

impl From<&Matrix4> for ModelMatrix {
    fn from(value: &Matrix4) -> Self {
        ModelMatrix(*value)
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct ModelNormalMatrix(Matrix4, Matrix3);

impl From<&Matrix4> for ModelNormalMatrix {
    fn from(value: &Matrix4) -> Self {
        let normal = <_ as Into<Matrix3>>::into(*value).inv().transpose();
        ModelNormalMatrix(*value, normal)
    }
}

impl PushConstant for ModelNormalMatrix {
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

pub type PipelineLayoutMaterial<M> = PipelineLayoutBuilder<
    Cons<<M as VulkanMaterial>::DescriptorLayout, Cons<CameraDescriptorSet, Nil>>,
    Cons<ModelNormalMatrix, Nil>,
>;

pub type PipelineLayoutSkybox<A> =
    PipelineLayoutBuilder<Cons<TextureDescriptorSet<A>, Nil>, Cons<CameraMatrices, Nil>>;

pub type PipelineLayoutNoMaterial =
    PipelineLayoutBuilder<Cons<CameraDescriptorSet, Nil>, Cons<ModelMatrix, Nil>>;

pub type PipelineLayoutGBuffer = PipelineLayoutBuilder<Cons<GBufferDescriptorSet, Nil>, Nil>;

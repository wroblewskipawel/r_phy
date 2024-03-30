use std::error::Error;
use std::path::Path;

use ash::vk;

use crate::{
    physics::shape,
    renderer::{camera::CameraMatrices, vulkan::device::pipeline::DescriptorLayoutBuilder},
};

use super::{
    descriptor::DescriptorPool,
    image::Texture2D,
    mesh::MeshPack,
    pipeline::{GraphicsPipeline, GraphicsPipelineLayoutTextured},
    render_pass::VulkanRenderPass,
    swapchain::VulkanSwapchain,
    VulkanDevice,
};

// TODO: Better handling of pipeline layout, register pipeline layout by type and create once
pub struct Skybox {
    texture: Texture2D,
    pub mesh_pack: MeshPack,
    pub descriptor: DescriptorPool<Texture2D>,
    pub pipeline: GraphicsPipeline<GraphicsPipelineLayoutTextured>,
}

impl VulkanDevice {
    pub fn create_skybox(
        &self,
        render_pass: &VulkanRenderPass,
        swapchain: &VulkanSwapchain,
        path: &Path,
    ) -> Result<Skybox, Box<dyn Error>> {
        let texture = self.load_cubemap(path)?;
        let mut descriptor =
            self.create_descriptor_pool(1, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)?;
        let pipeline_layout = self.create_graphics_pipeline_layout(
            DescriptorLayoutBuilder::new()
                .push::<CameraMatrices>()
                .push::<Texture2D>(),
        )?;
        self.write_image_samplers(&mut descriptor, std::slice::from_ref(&texture));
        let pipeline = self.create_graphics_pipeline(
            pipeline_layout,
            render_pass,
            swapchain,
            &[
                &Path::new("shaders/spv/skybox/vert.spv"),
                &Path::new("shaders/spv/skybox/frag.spv"),
            ],
            true,
        )?;
        let mesh_pack = self.load_mesh_pack(&[shape::Cube::new(1.0).into()])?;
        Ok(Skybox {
            texture,
            mesh_pack,
            descriptor,
            pipeline,
        })
    }

    pub fn destroy_skybox(&self, skybox: &mut Skybox) {
        self.destory_descriptor_pool(&mut skybox.descriptor);
        self.destory_texture(&mut skybox.texture);
        self.destory_graphics_pipeline(&mut skybox.pipeline);
        self.destory_mesh_pack(&mut skybox.mesh_pack);
    }
}

use std::error::Error;
use std::path::Path;

use crate::physics::shape;

use super::{
    descriptor::{DescriptorPool, TextureDescriptorSet},
    image::Texture2D,
    mesh::MeshPack,
    pipeline::{GraphicsPipeline, PipelineLayoutSkybox, PipelineStatesSkybox},
    render_pass::VulkanRenderPass,
    swapchain::VulkanSwapchain,
    VulkanDevice,
};

// TODO: Better handling of pipeline layout, register pipeline layout by type and create once
pub struct Skybox {
    texture: Texture2D,
    pub mesh_pack: MeshPack,
    pub descriptor: DescriptorPool<TextureDescriptorSet>,
    pub pipeline: GraphicsPipeline<PipelineLayoutSkybox, PipelineStatesSkybox>,
}

impl VulkanDevice {
    pub fn create_skybox(
        &self,
        render_pass: &VulkanRenderPass,
        swapchain: &VulkanSwapchain,
        path: &Path,
    ) -> Result<Skybox, Box<dyn Error>> {
        let texture = self.load_cubemap(path)?;
        let mut descriptor = self.create_descriptor_pool(TextureDescriptorSet::builder(), 1)?;
        let descriptor_write = descriptor
            .get_writer()
            .write_image(std::slice::from_ref(&texture));
        self.write_descriptor_sets(&mut descriptor, descriptor_write);
        let pipeline = self.create_graphics_pipeline(
            PipelineLayoutSkybox::builder(),
            PipelineStatesSkybox::builder(),
            render_pass,
            swapchain,
            &[
                &Path::new("shaders/spv/skybox/vert.spv"),
                &Path::new("shaders/spv/skybox/frag.spv"),
            ],
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
        self.destroy_descriptor_pool(&mut skybox.descriptor);
        self.destroy_texture(&mut skybox.texture);
        self.destroy_graphics_pipeline(&mut skybox.pipeline);
        self.destroy_mesh_pack(&mut skybox.mesh_pack);
    }
}

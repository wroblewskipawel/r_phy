use std::error::Error;
use std::path::Path;

use crate::{physics::shape, renderer::camera::CameraMatrices};

use super::{
    descriptor::{DescriptorPool, TextureDescriptorSet},
    image::Texture2D,
    mesh::MeshPack,
    pipeline::{
        DescriptorLayoutNode, DescriptorLayoutTerminator, GraphicsPipeline, GraphicspipelineConfig,
        ModuleLoader, PipelineLayoutBuilder, PushConstantNode, PushConstantTerminator,
    },
    VulkanDevice,
};

pub type LayoutSkybox = PipelineLayoutBuilder<
    DescriptorLayoutNode<TextureDescriptorSet, DescriptorLayoutTerminator>,
    PushConstantNode<CameraMatrices, PushConstantTerminator>,
>;

pub struct Skybox<L: GraphicspipelineConfig<Layout = LayoutSkybox>> {
    texture: Texture2D,
    pub mesh_pack: MeshPack,
    pub descriptor: DescriptorPool<TextureDescriptorSet>,
    pub pipeline: GraphicsPipeline<L>,
}

impl VulkanDevice {
    pub fn create_skybox<L: GraphicspipelineConfig<Layout = LayoutSkybox>>(
        &self,
        path: &Path,
        modules: impl ModuleLoader,
    ) -> Result<Skybox<L>, Box<dyn Error>> {
        let texture = self.load_cubemap(path)?;
        let mut descriptor = self.create_descriptor_pool(TextureDescriptorSet::builder(), 1)?;
        let descriptor_write = descriptor
            .get_writer()
            .write_image(std::slice::from_ref(&texture));
        self.write_descriptor_sets(&mut descriptor, descriptor_write);
        let image_extent = self.physical_device.surface_properties.get_current_extent();
        let pipeline = self.create_graphics_pipeline(modules, image_extent)?;
        let mesh_pack = self.load_mesh_pack(&[shape::Cube::new(1.0).into()])?;
        Ok(Skybox {
            texture,
            mesh_pack,
            descriptor,
            pipeline,
        })
    }

    pub fn destroy_skybox<L: GraphicspipelineConfig<Layout = LayoutSkybox>>(
        &self,
        skybox: &mut Skybox<L>,
    ) {
        self.destroy_descriptor_pool(&mut skybox.descriptor);
        self.destroy_texture(&mut skybox.texture);
        self.destroy_graphics_pipeline(&mut skybox.pipeline);
        self.destroy_mesh_pack(&mut skybox.mesh_pack);
    }
}

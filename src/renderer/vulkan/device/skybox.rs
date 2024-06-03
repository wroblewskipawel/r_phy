use std::error::Error;
use std::path::Path;

use crate::{
    physics::shape,
    renderer::{camera::CameraMatrices, model::CommonVertex},
};

use super::{
    descriptor::{DescriptorPool, DescriptorSetWriter, TextureDescriptorSet},
    image::Texture2D,
    pipeline::{
        DescriptorLayoutNode, DescriptorLayoutTerminator, GraphicsPipeline, GraphicsPipelineConfig,
        ModuleLoader, PipelineLayoutBuilder, PushConstantNode, PushConstantTerminator,
    },
    resources::{MeshPack, MeshPackRef, MeshPackTypeErased},
    VulkanDevice,
};

pub type LayoutSkybox = PipelineLayoutBuilder<
    DescriptorLayoutNode<TextureDescriptorSet, DescriptorLayoutTerminator>,
    PushConstantNode<CameraMatrices, PushConstantTerminator>,
>;

pub struct Skybox<L: GraphicsPipelineConfig<Layout = LayoutSkybox>> {
    texture: Texture2D,
    pub mesh_pack: MeshPack<CommonVertex>,
    pub descriptor: DescriptorPool<TextureDescriptorSet>,
    pub pipeline: GraphicsPipeline<L>,
}

impl VulkanDevice {
    pub fn create_skybox<L: GraphicsPipelineConfig<Layout = LayoutSkybox>>(
        &self,
        path: &Path,
        modules: impl ModuleLoader,
    ) -> Result<Skybox<L>, Box<dyn Error>> {
        let texture = self.load_cubemap(path)?;
        let descriptor = self.create_descriptor_pool(
            DescriptorSetWriter::<TextureDescriptorSet>::new(1)
                .write_images::<Texture2D, _>(std::slice::from_ref(&texture)),
        )?;
        let image_extent = self.physical_device.surface_properties.get_current_extent();
        let pipeline = self.create_graphics_pipeline(modules, image_extent)?;
        let mesh_pack = self.load_mesh_pack(&[shape::Cube::new(1.0).into()], usize::MAX)?;
        Ok(Skybox {
            texture,
            mesh_pack,
            descriptor,
            pipeline,
        })
    }

    pub fn destroy_skybox<L: GraphicsPipelineConfig<Layout = LayoutSkybox>>(
        &self,
        skybox: &mut Skybox<L>,
    ) {
        self.destroy_descriptor_pool(&mut skybox.descriptor);
        self.destroy_texture(&mut skybox.texture);
        self.destroy_pipeline(&mut skybox.pipeline);
        self.destroy_mesh_pack(&mut skybox.mesh_pack);
    }
}

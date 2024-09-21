use std::error::Error;
use std::path::Path;

use crate::{
    core::{Cons, Nil},
    physics::shape,
    renderer::{camera::CameraMatrices, model::CommonVertex},
};

use super::{
    descriptor::{DescriptorPool, DescriptorSetWriter, TextureDescriptorSet},
    image::Texture2D,
    memory::Allocator,
    pipeline::{GraphicsPipeline, GraphicsPipelineConfig, ModuleLoader, PipelineLayoutBuilder},
    resources::MeshPack,
    VulkanDevice,
};

pub type LayoutSkybox<A> =
    PipelineLayoutBuilder<Cons<TextureDescriptorSet<A>, Nil>, Cons<CameraMatrices, Nil>>;

pub struct Skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>> {
    texture: Texture2D<A>,
    pub mesh_pack: MeshPack<CommonVertex, A>,
    pub descriptor: DescriptorPool<TextureDescriptorSet<A>>,
    pub pipeline: GraphicsPipeline<L>,
}

impl VulkanDevice {
    pub fn create_skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>>(
        &mut self,
        allocator: &mut A,
        path: &Path,
        modules: impl ModuleLoader,
    ) -> Result<Skybox<A, L>, Box<dyn Error>> {
        let texture = self.load_cubemap(allocator, path)?;
        let descriptor = self.create_descriptor_pool(
            DescriptorSetWriter::<TextureDescriptorSet<A>>::new(1)
                .write_images::<Texture2D<A>, _>(std::slice::from_ref(&texture)),
        )?;
        let layout = self.get_pipeline_layout::<L::Layout>()?;
        let pipeline = self.create_graphics_pipeline(layout, &modules)?;
        let mesh_pack = self.load_mesh_pack(allocator, &[shape::Cube::new(1.0).into()])?;
        Ok(Skybox {
            texture,
            mesh_pack,
            descriptor,
            pipeline,
        })
    }

    pub fn destroy_skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>>(
        &self,
        skybox: &mut Skybox<A, L>,
        allocator: &mut A,
    ) {
        self.destroy_descriptor_pool(&mut skybox.descriptor);
        self.destroy_texture(&mut skybox.texture, allocator);
        self.destroy_pipeline(&mut skybox.pipeline);
        self.destroy_mesh_pack(&mut skybox.mesh_pack, allocator);
    }
}

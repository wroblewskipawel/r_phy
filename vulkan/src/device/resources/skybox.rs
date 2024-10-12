use std::error::Error;
use std::path::Path;

use physics::shape;
use to_resolve::{camera::CameraMatrices, model::CommonVertex};

use crate::device::{
    descriptor::{DescriptorPool, DescriptorSetWriter, TextureDescriptorSet},
    memory::Allocator,
    pipeline::{GraphicsPipeline, GraphicsPipelineConfig, ModuleLoader, PipelineLayoutBuilder},
    Device,
};
use type_list::{Cons, Nil};

use super::{
    image::{ImageReader, Texture2D},
    MeshPack,
};

pub type LayoutSkybox<A> =
    PipelineLayoutBuilder<Cons<TextureDescriptorSet<A>, Nil>, Cons<CameraMatrices, Nil>>;

pub struct Skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>> {
    cubemap: Texture2D<A>,
    pub mesh_pack: MeshPack<CommonVertex, A>,
    pub descriptor: DescriptorPool<TextureDescriptorSet<A>>,
    pub pipeline: GraphicsPipeline<L>,
}

impl Device {
    pub fn create_skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>>(
        &self,
        allocator: &mut A,
        path: &Path,
        modules: impl ModuleLoader,
    ) -> Result<Skybox<A, L>, Box<dyn Error>> {
        let cubemap = self.load_texture(allocator, ImageReader::cube(path)?)?;
        let descriptor = self.create_descriptor_pool(
            DescriptorSetWriter::<TextureDescriptorSet<A>>::new(1)
                .write_images::<Texture2D<A>, _>(std::slice::from_ref(&cubemap)),
        )?;
        let layout = self.get_pipeline_layout::<L::Layout>()?;
        let pipeline = self.create_graphics_pipeline(layout, &modules)?;
        let mesh_pack = self.load_mesh_pack(allocator, &[shape::Cube::new(1.0).into()])?;
        Ok(Skybox {
            cubemap,
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
        self.destroy_texture(&mut skybox.cubemap, allocator);
        self.destroy_pipeline(&mut skybox.pipeline);
        self.destroy_mesh_pack(&mut skybox.mesh_pack, allocator);
    }
}

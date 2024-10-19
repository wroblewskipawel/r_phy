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
use type_kit::{Cons, Destroy, DropGuard, Nil};

use super::{
    image::{ImageReader, Texture2D},
    MeshPack,
};

pub type LayoutSkybox<A> =
    PipelineLayoutBuilder<Cons<TextureDescriptorSet<A>, Nil>, Cons<CameraMatrices, Nil>>;

pub struct Skybox<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>> {
    cubemap: DropGuard<Texture2D<A>>,
    pub mesh_pack: DropGuard<MeshPack<CommonVertex, A>>,
    pub descriptor: DropGuard<DescriptorPool<TextureDescriptorSet<A>>>,
    pub pipeline: DropGuard<GraphicsPipeline<L>>,
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
            cubemap: DropGuard::new(cubemap),
            mesh_pack: DropGuard::new(mesh_pack),
            descriptor: DropGuard::new(descriptor),
            pipeline: DropGuard::new(pipeline),
        })
    }
}

impl<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>> Destroy for Skybox<A, L> {
    type Context<'a> = (&'a Device, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        self.descriptor.destroy(device);
        self.mesh_pack.destroy((device, allocator));
        self.cubemap.destroy((device, allocator));
        self.pipeline.destroy(device);
    }
}

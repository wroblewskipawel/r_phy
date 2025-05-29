use std::{cell::RefCell, convert::Infallible, path::Path};

use graphics::{model::CommonVertex, renderer::camera::CameraMatrices};
use physics::shape;

use crate::context::{
    device::{
        descriptor::{DescriptorPool, DescriptorSetWriter, TextureDescriptorSet},
        memory::Allocator,
        pipeline::{
            GraphicsPipeline, GraphicsPipelineConfig, PipelineLayoutBuilder, ShaderDirectory,
        },
        Device,
    },
    error::VkError,
};
use type_kit::{Cons, Create, Destroy, DestroyResult, DropGuard, DropGuardError, Nil};

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

const SKYBOX_SHADER: &'static str = "_resources/shaders/spv/skybox";

impl<A: Allocator, L: GraphicsPipelineConfig<Layout = LayoutSkybox<A>>> Create for Skybox<A, L> {
    type Config<'a> = &'a Path;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let cubemap = device.load_texture(allocator, ImageReader::cube(config)?)?;
        let descriptor = DescriptorPool::create(
            DescriptorSetWriter::<TextureDescriptorSet<A>>::new(1)
                .write_images::<Texture2D<A>, _>(std::slice::from_ref(&cubemap)),
            device,
        )?;
        let layout = device.get_pipeline_layout::<L::Layout>()?;
        let modules = ShaderDirectory::new(Path::new(SKYBOX_SHADER));
        let pipeline = GraphicsPipeline::create((layout, &modules), device)?;
        let mesh_pack = device.load_mesh_pack(allocator, &[shape::Cube::new(1.0).into()])?;
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
    type DestroyError = DropGuardError<Infallible>;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        let (device, allocator) = context;
        self.descriptor.destroy(device)?;
        self.mesh_pack.destroy((device, &RefCell::new(allocator)))?;
        self.cubemap.destroy((device, allocator))?;
        self.pipeline.destroy(device)?;
        Ok(())
    }
}

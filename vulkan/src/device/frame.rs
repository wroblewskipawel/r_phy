// Temporary allow for too_many_arguments
// handling render commands will be significantly revamped in the future
// which makes it not worth the effort to refactor this code
#![allow(clippy::too_many_arguments)]

use std::{error::Error, marker::PhantomData};

use to_resolve::{
    camera::CameraMatrices,
    model::Drawable,
    shader::{ShaderHandle, ShaderType},
};
use type_kit::{Destroy, DestroyCollection, DropGuard};

use crate::Context;
use math::types::Matrix4;

use super::{
    command::{
        level::{Primary, Secondary},
        operation::Graphics,
        BeginCommand, Persistent, PersistentCommandPool,
    },
    descriptor::{CameraDescriptorSet, Descriptor, DescriptorPool, DescriptorSetWriter},
    framebuffer::AttachmentList,
    memory::{Allocator, DefaultAllocator},
    pipeline::{
        GraphicsPipelineConfig, GraphicsPipelineListBuilder, GraphicsPipelinePackList, ModuleLoader,
    },
    resources::{
        buffer::{UniformBuffer, UniformBufferBuilder, UniformBufferPartial},
        MaterialPackList, MeshPackList, PartialBuilder,
    },
    swapchain::{Swapchain, SwapchainFrame, SwapchainImageSync},
    Device,
};

pub trait Frame: 'static {
    type Shader<S: ShaderType>: ShaderType + GraphicsPipelineConfig + ModuleLoader;
    type Context<P: GraphicsPipelinePackList>: FrameContext
        + for<'a> Destroy<Context<'a> = &'a Context>;

    fn load_context<P: GraphicsPipelinePackList>(
        &self,
        context: &Context,
        pipelines: &impl GraphicsPipelineListBuilder<Pack = P>,
    ) -> Result<Self::Context<P>, Box<dyn Error>>;
}

pub trait FrameContext: Sized {
    const REQUIRED_COMMANDS: usize;
    type Attachments: AttachmentList;
    type State;

    fn begin_frame(
        &mut self,
        device: &Device,
        camera: &CameraMatrices,
    ) -> Result<(), Box<dyn Error>>;

    fn draw<
        A1: Allocator,
        A2: Allocator,
        S: ShaderType,
        D: Drawable<Material = S::Material, Vertex = S::Vertex>,
        M: MaterialPackList<A2>,
        V: MeshPackList<A1>,
    >(
        &mut self,
        shader: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
        material_packs: &M,
        mesh_packs: &V,
    );

    fn end_frame(&mut self, device: &Device) -> Result<(), Box<dyn Error>>;
}

pub struct CameraUniform {
    pub descriptors: DropGuard<DescriptorPool<CameraDescriptorSet>>,
    pub uniform_buffer: DropGuard<UniformBuffer<CameraMatrices, Graphics, DefaultAllocator>>,
}

pub struct FrameData<C: FrameContext> {
    pub swapchain_frame: SwapchainFrame<C::Attachments>,
    pub primary_command: BeginCommand<Persistent, Primary, Graphics>,
    pub camera_descriptor: Descriptor<CameraDescriptorSet>,
    pub renderer_state: C::State,
}

pub struct FramePool<F: FrameContext> {
    pub image_sync: Vec<SwapchainImageSync>,
    pub camera_uniform: CameraUniform,
    pub primary_commands: PersistentCommandPool<Primary, Graphics>,
    pub secondary_commands: PersistentCommandPool<Secondary, Graphics>,
    _phantom: PhantomData<F>,
}

impl Context {
    fn create_camera_uniform(&self, num_images: usize) -> Result<CameraUniform, Box<dyn Error>> {
        let uniform_buffer =
            UniformBufferPartial::prepare(UniformBufferBuilder::new(num_images), self)?
                .finalize(self, &mut DefaultAllocator {})?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<CameraDescriptorSet>::new(num_images)
                .write_buffer(&uniform_buffer),
        )?;
        Ok(CameraUniform {
            descriptors: DropGuard::new(descriptors),
            uniform_buffer: DropGuard::new(uniform_buffer),
        })
    }

    pub fn create_frame_pool<F: FrameContext>(
        &self,
        swapchain: &Swapchain<F::Attachments>,
    ) -> Result<FramePool<F>, Box<dyn Error>> {
        let image_sync = self.create_swapchain_image_sync(swapchain)?;
        let primary_commands = self.create_persistent_command_pool(swapchain.num_images)?;
        let secondary_commands =
            self.create_persistent_command_pool(swapchain.num_images * F::REQUIRED_COMMANDS)?;
        let camera_uniform = self.create_camera_uniform(swapchain.num_images)?;

        Ok(FramePool {
            image_sync,
            camera_uniform,
            primary_commands,
            secondary_commands,
            _phantom: PhantomData,
        })
    }
}

impl Destroy for CameraUniform {
    type Context<'a> = &'a Device;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        self.descriptors.destroy(context);
        self.uniform_buffer
            .destroy((context, &mut DefaultAllocator {}));
    }
}

impl<F: FrameContext> Destroy for FramePool<F> {
    type Context<'a> = &'a Context;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        self.image_sync.iter_mut().destroy(context);
        self.primary_commands.destroy(context);
        self.secondary_commands.destroy(context);
        self.camera_uniform.destroy(context);
    }
}

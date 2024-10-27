// Temporary allow for too_many_arguments
// handling render commands will be significantly revamped in the future
// which makes it not worth the effort to refactor this code
#![allow(clippy::too_many_arguments)]

use std::{cell::RefCell, convert::Infallible, error::Error, marker::PhantomData};

use to_resolve::{
    camera::CameraMatrices,
    model::Drawable,
    shader::{ShaderHandle, ShaderType},
};
use type_kit::{
    Create, CreateCollection, CreateResult, Destroy, DestroyCollection, DestroyResult, DropGuard,
    DropGuardError,
};

use crate::{error::VkError, Context};
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
        + for<'a> Create<Context<'a> = &'a Context>;

    fn load_context<P: GraphicsPipelinePackList>(
        &self,
        context: &Context,
        pipelines: &impl GraphicsPipelineListBuilder<Pack = P>,
    ) -> CreateResult<Self::Context<P>>;
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

impl Create for CameraUniform {
    type Config<'a> = usize;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let buffer_partial =
            UniformBufferPartial::prepare(UniformBufferBuilder::new(config), &context)?;
        let uniform_buffer = UniformBuffer::create(
            buffer_partial,
            (context, &RefCell::new(&mut DefaultAllocator {})),
        )?;
        let descriptors = DescriptorPool::create(
            DescriptorSetWriter::<CameraDescriptorSet>::new(config).write_buffer(&uniform_buffer),
            context,
        )?;
        Ok(CameraUniform {
            descriptors: DropGuard::new(descriptors),
            uniform_buffer: DropGuard::new(uniform_buffer),
        })
    }
}

impl Destroy for CameraUniform {
    type Context<'a> = &'a Device;
    type DestroyError = DropGuardError<Infallible>;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.descriptors.destroy(context)?;
        self.uniform_buffer
            .destroy((context, &RefCell::new(&mut DefaultAllocator {})))?;
        Ok(())
    }
}

impl<F: FrameContext> Create for FramePool<F> {
    type Config<'a> = &'a Swapchain<F::Attachments>;
    type CreateError = VkError;

    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let image_sync = (0..config.num_images)
            .map(|_| ())
            .create(context)
            .collect::<Result<Vec<_>, _>>()?;
        let primary_commands = PersistentCommandPool::create(config.num_images, context)?;
        let secondary_commands =
            PersistentCommandPool::create(config.num_images * F::REQUIRED_COMMANDS, context)?;
        let camera_uniform = CameraUniform::create(config.num_images, context)?;

        Ok(FramePool {
            image_sync,
            camera_uniform,
            primary_commands,
            secondary_commands,
            _phantom: PhantomData,
        })
    }
}

impl<F: FrameContext> Destroy for FramePool<F> {
    type Context<'a> = &'a Device;
    type DestroyError = DropGuardError<Infallible>;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.image_sync.iter_mut().destroy(context)?;
        self.primary_commands.destroy(context)?;
        self.secondary_commands.destroy(context)?;
        self.camera_uniform.destroy(context)?;
        Ok(())
    }
}

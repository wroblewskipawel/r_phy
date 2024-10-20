mod commands;
mod draw_graph;

use std::{cell::RefCell, error::Error, path::Path, rc::Rc};

use ash::vk;

use commands::Commands;
use draw_graph::DrawGraph;

use to_resolve::{
    camera::CameraMatrices,
    model::{CommonVertex, Drawable, MeshBuilder},
    shader::{ShaderHandle, ShaderType},
};
use type_kit::{Create, CreateResult, Destroy, DropGuard};

use crate::{
    device::{
        descriptor::{DescriptorPool, DescriptorSetWriter, GBufferDescriptorSet},
        frame::{Frame, FrameContext, FrameData, FramePool},
        framebuffer::{
            presets::AttachmentsGBuffer, AttachmentReferences, AttachmentsBuilder, Builder,
            InputAttachment,
        },
        memory::{Allocator, DeviceLocal},
        pipeline::{
            GBufferDepthPrepasPipeline, GBufferShadingPassPipeline, GBufferSkyboxPipeline,
            GraphicsPipeline, GraphicsPipelineConfig, GraphicsPipelineListBuilder,
            GraphicsPipelinePackList, ModuleLoader, Modules, PipelineLayoutMaterial,
            ShaderDirectory, StatesDepthWriteDisabled,
        },
        render_pass::{
            DeferedRenderPass, GBufferShadingPass, GBufferWritePass, RenderPass, Subpass,
        },
        resources::{image::Image2D, MaterialPackList, MeshPack, MeshPackList, Skybox},
        swapchain::Swapchain,
        Device,
    },
    error::{ShaderResult, VkError},
    Context,
};

use math::types::{Matrix4, Vector3};

pub struct DeferredShader<S: ShaderType> {
    shader: S,
}

impl<S: ShaderType> ShaderType for DeferredShader<S> {
    type Material = S::Material;
    type Vertex = S::Vertex;

    fn source(&self) -> &Path {
        self.shader.source()
    }
}
impl<S: ShaderType> GraphicsPipelineConfig for DeferredShader<S> {
    type Attachments = AttachmentsGBuffer;
    type Layout = PipelineLayoutMaterial<S::Material>;
    type PipelineStates = StatesDepthWriteDisabled<S::Vertex>;
    type RenderPass = DeferedRenderPass<AttachmentsGBuffer>;
    type Subpass = GBufferWritePass<AttachmentsGBuffer>;
}

impl<S: ShaderType> From<S> for DeferredShader<S> {
    fn from(shader: S) -> Self {
        DeferredShader { shader }
    }
}

impl<S: ShaderType> ModuleLoader for DeferredShader<S> {
    fn load<'a>(&self, device: &'a Device) -> ShaderResult<Modules<'a>> {
        ShaderDirectory::new(self.shader.source()).load(device)
    }
}

pub struct GBuffer<A: Allocator> {
    pub combined: DropGuard<Image2D<DeviceLocal, A>>,
    pub albedo: DropGuard<Image2D<DeviceLocal, A>>,
    pub normal: DropGuard<Image2D<DeviceLocal, A>>,
    pub position: DropGuard<Image2D<DeviceLocal, A>>,
    pub depth: DropGuard<Image2D<DeviceLocal, A>>,
}

struct DeferredRendererPipelines<P: GraphicsPipelinePackList> {
    write_pass: P,
    depth_prepass: DropGuard<GraphicsPipeline<GBufferDepthPrepasPipeline<AttachmentsGBuffer>>>,
    shading_pass: DropGuard<GraphicsPipeline<GBufferShadingPassPipeline<AttachmentsGBuffer>>>,
}

struct DeferredRendererFrameData<A: Allocator> {
    g_buffer: DropGuard<GBuffer<A>>,
    swapchain: DropGuard<Swapchain<AttachmentsGBuffer>>,
    descriptors: DescriptorPool<GBufferDescriptorSet>,
}

struct DeferredRendererResources<A: Allocator> {
    mesh: DropGuard<MeshPack<CommonVertex, A>>,
    skybox: DropGuard<Skybox<A, GBufferSkyboxPipeline<AttachmentsGBuffer, A>>>,
}

pub struct DeferredRendererContext<A: Allocator, P: GraphicsPipelinePackList> {
    renderer: Rc<RefCell<DropGuard<DeferredRenderer<A>>>>,
    pipelines: DeferredRendererPipelines<P>,
    frames: FramePool<Self>,
    current_frame: Option<FrameData<Self>>,
}

pub struct DeferredRendererFrameState<P: GraphicsPipelinePackList> {
    commands: Commands<P>,
    draw_graph: DrawGraph,
}

pub struct DeferredRenderer<A: Allocator> {
    render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
    frame_data: DropGuard<DeferredRendererFrameData<A>>,
    resources: DropGuard<DeferredRendererResources<A>>,
}

impl<A: Allocator> Frame for Rc<RefCell<DropGuard<DeferredRenderer<A>>>> {
    type Shader<S: ShaderType> = DeferredShader<S>;
    type Context<P: GraphicsPipelinePackList> = DeferredRendererContext<A, P>;

    fn load_context<P: GraphicsPipelinePackList>(
        &self,
        context: &Context,
        pipelines: &impl GraphicsPipelineListBuilder<Pack = P>,
    ) -> CreateResult<Self::Context<P>> {
        let renderer = self.clone();
        let pipelines = pipelines.build(context)?;
        DeferredRendererContext::create((renderer, pipelines), context)
    }
}

impl<A: Allocator, P: GraphicsPipelinePackList> FrameContext for DeferredRendererContext<A, P> {
    const REQUIRED_COMMANDS: usize = P::LEN + 3;
    type Attachments = AttachmentsGBuffer;
    type State = DeferredRendererFrameState<P>;

    fn begin_frame(
        &mut self,
        device: &Device,
        camera_matrices: &CameraMatrices,
    ) -> Result<(), Box<dyn Error>> {
        let (index, primary_command) = self.frames.primary_commands.next();
        let primary_command = device.begin_primary_command(primary_command)?;
        let swapchain_frame = self
            .renderer
            .borrow()
            .frame_data
            .swapchain
            .get_frame(self.frames.image_sync[index])?;
        let camera_descriptor = self.frames.camera_uniform.descriptors.get(index);
        self.frames.camera_uniform.uniform_buffer[index] = *camera_matrices;
        let commands =
            self.prepare_commands(device, &swapchain_frame, camera_descriptor, camera_matrices)?;
        let draw_graph = DrawGraph::new();
        self.current_frame.replace(FrameData {
            swapchain_frame,
            primary_command,
            camera_descriptor,
            renderer_state: DeferredRendererFrameState {
                commands,
                draw_graph,
            },
        });
        Ok(())
    }

    fn draw<
        T1: Allocator,
        T2: Allocator,
        S: ShaderType,
        D: Drawable<Material = S::Material, Vertex = S::Vertex>,
        M: MaterialPackList<T2>,
        V: MeshPackList<T1>,
    >(
        &mut self,
        shader: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
        material_packs: &M,
        mesh_packs: &V,
    ) {
        self.append_draw_call(material_packs, mesh_packs, shader, drawable, transform);
    }

    fn end_frame(&mut self, device: &Device) -> Result<(), Box<dyn Error>> {
        let FrameData {
            swapchain_frame,
            primary_command,
            renderer_state,
            ..
        } = self.current_frame.take().ok_or("current_frame is None!")?;
        let commands = self.record_draw_calls(device, renderer_state, &swapchain_frame)?;
        let primary_command =
            self.record_primary_command(device, primary_command, commands, &swapchain_frame)?;
        let renderer = self.renderer.borrow();
        device.present_frame(
            &renderer.frame_data.swapchain,
            primary_command,
            swapchain_frame,
        )?;
        Ok(())
    }
}

impl<A: Allocator> GBuffer<A> {
    pub fn get_framebuffer_builder(
        &self,
        swapchain_image: vk::ImageView,
    ) -> Builder<AttachmentsGBuffer> {
        AttachmentsBuilder::new()
            .push(swapchain_image)
            .push(self.depth.image_view)
            .push(self.position.image_view)
            .push(self.normal.image_view)
            .push(self.albedo.image_view)
            .push(self.combined.image_view)
    }
}

impl<A: Allocator> Create for GBuffer<A> {
    type Config<'a> = ();
    type CreateError = VkError;

    fn create<'a, 'b>(
        _: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let combined = device.create_color_attachment_image(allocator)?;
        let albedo = device.create_color_attachment_image(allocator)?;
        let normal = device.create_color_attachment_image(allocator)?;
        let position = device.create_color_attachment_image(allocator)?;
        let depth = device.create_depth_stencil_attachment_image(allocator)?;
        Ok(GBuffer {
            combined: DropGuard::new(combined),
            albedo: DropGuard::new(albedo),
            normal: DropGuard::new(normal),
            position: DropGuard::new(position),
            depth: DropGuard::new(depth),
        })
    }
}

impl<A: Allocator> Destroy for GBuffer<A> {
    type Context<'a> = (&'a Device, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        self.combined.destroy((device, allocator));
        self.albedo.destroy((device, allocator));
        self.normal.destroy((device, allocator));
        self.position.destroy((device, allocator));
        self.depth.destroy((device, allocator));
    }
}

impl<A: Allocator> Create for DeferredRendererFrameData<A> {
    type Config<'a> = ();
    type CreateError = VkError;

    fn create<'a, 'b>(
        _: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let g_buffer = GBuffer::create((), (device, allocator))?;
        let framebuffer_builder = |swapchain_image, extent| {
            device.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                g_buffer.get_framebuffer_builder(swapchain_image),
                extent,
            )
        };
        let swapchain = Swapchain::create(&framebuffer_builder, device)?;
        let descriptors = DescriptorPool::create(
            DescriptorSetWriter::<GBufferDescriptorSet>::new(1).write_images::<InputAttachment, _>(
                &GBufferShadingPass::<AttachmentsGBuffer>::references()
                    .get_input_attachments(&swapchain.framebuffers[0]),
            ),
            device,
        )?;
        Ok(DeferredRendererFrameData {
            g_buffer: DropGuard::new(g_buffer),
            descriptors,
            swapchain: DropGuard::new(swapchain),
        })
    }
}

impl<A: Allocator> Destroy for DeferredRendererFrameData<A> {
    type Context<'a> = (&'a Context, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        self.descriptors.destroy(device);
        self.swapchain.destroy(device);
        self.g_buffer.destroy((device, allocator));
    }
}

impl<A: Allocator> Create for DeferredRendererResources<A> {
    type Config<'a> = ();
    type CreateError = VkError;

    fn create<'a, 'b>(
        _: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let skybox = Skybox::create(
            Path::new("_resources/assets/skybox/skybox"),
            (device, allocator),
        )?;
        let mesh = device.load_mesh_pack(
            allocator,
            &[MeshBuilder::plane_subdivided(
                0,
                2.0 * Vector3::y(),
                2.0 * Vector3::x(),
                Vector3::zero(),
                false,
            )
            .offset(Vector3::new(-1.0, -1.0, 0.0))
            .build()],
        )?;

        Ok(DeferredRendererResources {
            mesh: DropGuard::new(mesh),
            skybox: DropGuard::new(skybox),
        })
    }
}

impl<A: Allocator> Destroy for DeferredRendererResources<A> {
    type Context<'a> = (&'a Device, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        self.mesh.destroy((device, allocator));
        self.skybox.destroy((device, allocator));
    }
}

impl<P: GraphicsPipelinePackList> Create for DeferredRendererPipelines<P> {
    type Config<'a> = P;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let depth_prepass = GraphicsPipeline::create(
            (
                context.get_pipeline_layout()?,
                &ShaderDirectory::new(Path::new("_resources/shaders/spv/deferred/depth_prepass")),
            ),
            context,
        )?;
        let shading_pass = GraphicsPipeline::create(
            (
                context.get_pipeline_layout()?,
                &ShaderDirectory::new(Path::new("_resources/shaders/spv/deferred/gbuffer_combine")),
            ),
            context,
        )?;
        Ok(DeferredRendererPipelines {
            write_pass: config,
            depth_prepass: DropGuard::new(depth_prepass),
            shading_pass: DropGuard::new(shading_pass),
        })
    }
}

impl<P: GraphicsPipelinePackList> Destroy for DeferredRendererPipelines<P> {
    type Context<'a> = &'a Device;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        self.write_pass.destroy(context);
        self.depth_prepass.destroy(context);
        self.shading_pass.destroy(context);
    }
}

impl<A: Allocator> Create for DeferredRenderer<A> {
    type Config<'a> = ();
    type CreateError = VkError;

    fn create<'a, 'b>(
        _: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (context, allocator) = context;
        let render_pass = context.get_render_pass()?;
        let frame_data = DeferredRendererFrameData::create((), (context, allocator))?;
        let resources = DeferredRendererResources::create((), (context, allocator))?;
        Ok(DeferredRenderer {
            render_pass,
            frame_data: DropGuard::new(frame_data),
            resources: DropGuard::new(resources),
        })
    }
}

impl<A: Allocator> Destroy for DeferredRenderer<A> {
    type Context<'a> = (&'a Context, &'a mut A);

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        let (device, allocator) = context;
        self.frame_data.destroy((device, allocator));
        self.resources.destroy((device, allocator));
    }
}

impl<A: Allocator, P: GraphicsPipelinePackList> Create for DeferredRendererContext<A, P> {
    type Config<'a> = (Rc<RefCell<DropGuard<DeferredRenderer<A>>>>, P);
    type CreateError = VkError;

    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let (renderer, pipelines) = config;
        let (pipelines, frames) = {
            let renderer = renderer.borrow();
            (
                DeferredRendererPipelines::create(pipelines, context)?,
                FramePool::create(&renderer.frame_data.swapchain, context)?,
            )
        };
        Ok(DeferredRendererContext {
            renderer: renderer.clone(),
            pipelines,
            frames,
            current_frame: None,
        })
    }
}

impl<A: Allocator, P: GraphicsPipelinePackList> Destroy for DeferredRendererContext<A, P> {
    type Context<'a> = &'a Context;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        self.pipelines.destroy(context);
        self.frames.destroy(context);
    }
}

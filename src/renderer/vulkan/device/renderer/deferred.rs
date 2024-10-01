mod commands;
mod draw_graph;

use std::{cell::RefCell, error::Error, path::Path, rc::Rc};

use ash::vk;

use commands::Commands;
use draw_graph::DrawGraph;

use crate::{
    math::types::{Matrix4, Vector3},
    renderer::{
        camera::CameraMatrices,
        model::{CommonVertex, Drawable, MeshBuilder},
        shader::{ShaderHandle, ShaderType},
        vulkan::{
            core::Context,
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
                resources::{
                    image::VulkanImage2D, MaterialPackList, MeshPack, MeshPackList, Skybox,
                },
                swapchain::VulkanSwapchain,
                VulkanDevice,
            },
        },
    },
};

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
    fn load<'a>(&self, device: &'a VulkanDevice) -> Result<Modules<'a>, Box<dyn Error>> {
        ShaderDirectory::new(self.shader.source()).load(device)
    }
}

pub struct GBuffer<A: Allocator> {
    pub combined: VulkanImage2D<DeviceLocal, A>,
    pub albedo: VulkanImage2D<DeviceLocal, A>,
    pub normal: VulkanImage2D<DeviceLocal, A>,
    pub position: VulkanImage2D<DeviceLocal, A>,
    pub depth: VulkanImage2D<DeviceLocal, A>,
}

struct DeferredRendererPipelines<P: GraphicsPipelinePackList> {
    write_pass: P,
    depth_prepass: GraphicsPipeline<GBufferDepthPrepasPipeline<AttachmentsGBuffer>>,
    shading_pass: GraphicsPipeline<GBufferShadingPassPipeline<AttachmentsGBuffer>>,
}

struct DeferredRendererFrameData<A: Allocator> {
    g_buffer: GBuffer<A>,
    swapchain: VulkanSwapchain<AttachmentsGBuffer>,
    descriptors: DescriptorPool<GBufferDescriptorSet>,
}

struct DeferredRendererResources<A: Allocator> {
    mesh: MeshPack<CommonVertex, A>,
    skybox: Skybox<A, GBufferSkyboxPipeline<AttachmentsGBuffer, A>>,
}

pub struct DeferredRendererContext<A: Allocator, P: GraphicsPipelinePackList> {
    renderer: Rc<RefCell<DeferredRenderer<A>>>,
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
    frame_data: DeferredRendererFrameData<A>,
    resources: DeferredRendererResources<A>,
}

impl<A: Allocator> Frame for Rc<RefCell<DeferredRenderer<A>>> {
    type Shader<S: ShaderType> = DeferredShader<S>;
    type Context<P: GraphicsPipelinePackList> = DeferredRendererContext<A, P>;

    fn load_context<P: GraphicsPipelinePackList>(
        &self,
        context: &Context,
        pipelines: &impl GraphicsPipelineListBuilder<Pack = P>,
    ) -> Result<Self::Context<P>, Box<dyn Error>> {
        let borrow = self.borrow();
        let pipelines = context.create_pipelines(pipelines)?;
        let frames = context.create_frame_pool(&borrow.frame_data.swapchain)?;
        Ok(DeferredRendererContext {
            renderer: self.clone(),
            pipelines,
            frames,
            current_frame: None,
        })
    }

    fn destroy_context<P: GraphicsPipelinePackList>(
        renderer_context: &mut Self::Context<P>,
        context: &Context,
    ) {
        context.destroy_pipelines(&mut renderer_context.pipelines);
        context.destroy_frame_pool(&mut renderer_context.frames);
    }
}

impl<A: Allocator, P: GraphicsPipelinePackList> FrameContext for DeferredRendererContext<A, P> {
    const REQUIRED_COMMANDS: usize = P::LEN + 3;
    type Attachments = AttachmentsGBuffer;
    type State = DeferredRendererFrameState<P>;

    fn begin_frame(
        &mut self,
        device: &VulkanDevice,
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

    fn end_frame(&mut self, device: &VulkanDevice) -> Result<(), Box<dyn Error>> {
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

impl Context {
    fn create_pipelines<P: GraphicsPipelineListBuilder>(
        &self,
        pipelines: &P,
    ) -> Result<DeferredRendererPipelines<P::Pack>, Box<dyn Error>> {
        let write_pass = pipelines.build(&self)?;
        let depth_prepass = self.create_graphics_pipeline(
            self.get_pipeline_layout()?,
            &ShaderDirectory::new(Path::new("shaders/spv/deferred/depth_prepass")),
        )?;
        let shading_pass = self.create_graphics_pipeline(
            self.get_pipeline_layout()?,
            &ShaderDirectory::new(Path::new("shaders/spv/deferred/gbuffer_combine")),
        )?;
        Ok(DeferredRendererPipelines {
            write_pass,
            depth_prepass,
            shading_pass,
        })
    }

    fn create_frame_data<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<DeferredRendererFrameData<A>, Box<dyn Error>> {
        let g_buffer = self.create_g_buffer(allocator)?;
        let swapchain = self.create_swapchain(|swapchain_image, extent| {
            self.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                g_buffer.get_framebuffer_builder(swapchain_image),
                extent,
            )
        })?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<GBufferDescriptorSet>::new(1).write_images::<InputAttachment, _>(
                &GBufferShadingPass::<AttachmentsGBuffer>::references()
                    .get_input_attachments(&swapchain.framebuffers[0]),
            ),
        )?;
        Ok(DeferredRendererFrameData {
            g_buffer,
            descriptors,
            swapchain,
        })
    }

    fn create_renderer_resources<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<DeferredRendererResources<A>, Box<dyn Error>> {
        let skybox = self.create_skybox(
            allocator,
            Path::new("assets/skybox/skybox"),
            ShaderDirectory::new(Path::new("shaders/spv/skybox")),
        )?;
        let mesh = self.load_mesh_pack(
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

        Ok(DeferredRendererResources { mesh, skybox })
    }

    pub fn create_deferred_renderer<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<DeferredRenderer<A>, Box<dyn Error>> {
        let frame_data = self.create_frame_data(allocator)?;
        let render_pass = self.get_render_pass()?;
        let resources = self.create_renderer_resources(allocator)?;
        Ok(DeferredRenderer {
            frame_data,
            render_pass,
            resources,
        })
    }

    fn destroy_pipelines<P: GraphicsPipelinePackList>(
        &self,
        pipelines: &mut DeferredRendererPipelines<P>,
    ) {
        pipelines.write_pass.destroy(&self);
        self.destroy_pipeline(&mut pipelines.depth_prepass);
        self.destroy_pipeline(&mut pipelines.shading_pass);
    }

    fn destroy_frame_state<A: Allocator>(
        &self,
        frames: &mut DeferredRendererFrameData<A>,
        allocator: &mut A,
    ) {
        self.destroy_descriptor_pool(&mut frames.descriptors);
        self.destroy_swapchain(&mut frames.swapchain);
        self.destroy_g_buffer(&mut frames.g_buffer, allocator);
    }

    fn destroy_renderer_resources<A: Allocator>(
        &self,
        resources: &mut DeferredRendererResources<A>,
        allocator: &mut A,
    ) {
        self.destroy_mesh_pack(&mut resources.mesh, allocator);
        self.destroy_skybox(&mut resources.skybox, allocator);
    }

    pub fn destroy_deferred_renderer<A: Allocator>(
        &self,
        renderer: &mut DeferredRenderer<A>,
        allocator: &mut A,
    ) {
        self.destroy_frame_state(&mut renderer.frame_data, allocator);
        self.destroy_renderer_resources(&mut renderer.resources, allocator);
    }

    fn create_g_buffer<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<GBuffer<A>, Box<dyn Error>> {
        let combined = self.create_color_attachment_image(allocator)?;
        let albedo = self.create_color_attachment_image(allocator)?;
        let normal = self.create_color_attachment_image(allocator)?;
        let position = self.create_color_attachment_image(allocator)?;
        let depth = self.create_depth_stencil_attachment_image(allocator)?;
        Ok(GBuffer {
            combined,
            albedo,
            normal,
            position,
            depth,
        })
    }

    fn destroy_g_buffer<A: Allocator>(&self, g_buffer: &mut GBuffer<A>, allocator: &mut A) {
        self.destroy_image(&mut g_buffer.combined, allocator);
        self.destroy_image(&mut g_buffer.albedo, allocator);
        self.destroy_image(&mut g_buffer.normal, allocator);
        self.destroy_image(&mut g_buffer.position, allocator);
        self.destroy_image(&mut g_buffer.depth, allocator);
    }
}

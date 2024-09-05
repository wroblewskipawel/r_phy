mod commands;
mod draw_graph;

use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use ash::vk;

use commands::Commands;
use draw_graph::DrawGraph;

use crate::{
    math::types::{Matrix4, Vector3},
    renderer::{
        camera::CameraMatrices,
        model::{CommonVertex, Drawable, MeshBuilder},
        shader::{ShaderHandle, ShaderType, ShaderTypeList},
        vulkan::{
            core::Context,
            device::{
                descriptor::{DescriptorPool, DescriptorSetWriter, GBufferDescriptorSet},
                frame::{Frame, FrameData, FramePool},
                framebuffer::{
                    presets::AttachmentsGBuffer, AttachmentReferences, AttachmentsBuilder, Builder,
                    InputAttachment,
                },
                image::VulkanImage2D,
                pipeline::{
                    GBufferDepthPrepasPipeline, GBufferShadingPassPipeline, GBufferSkyboxPipeline,
                    GBufferWritePassPipeline, GraphicsPipeline, GraphicsPipelineTypeList,
                    PipelineCollection, ShaderDirectory,
                },
                render_pass::{DeferedRenderPass, GBufferShadingPass, RenderPass, Subpass},
                resources::{MaterialPackList, MeshPack, MeshPackList},
                skybox::Skybox,
                swapchain::VulkanSwapchain,
                VulkanDevice,
            },
        },
    },
};

impl<S: ShaderTypeList> GraphicsPipelineTypeList for S {
    const LEN: usize = S::LEN;
    type Pipeline = GBufferWritePassPipeline<
        AttachmentsGBuffer,
        <S::Item as ShaderType>::Material,
        <S::Item as ShaderType>::Vertex,
    >;
    type Next = S::Next;
}

pub struct GBuffer {
    pub combined: VulkanImage2D,
    pub albedo: VulkanImage2D,
    pub normal: VulkanImage2D,
    pub position: VulkanImage2D,
    pub depth: VulkanImage2D,
}

pub struct Pipelines<S: ShaderTypeList> {
    write_pass: PipelineCollection,
    depth_prepass: GraphicsPipeline<GBufferDepthPrepasPipeline<AttachmentsGBuffer>>,
    shading_pass: GraphicsPipeline<GBufferShadingPassPipeline<AttachmentsGBuffer>>,
    _phantom: PhantomData<S>,
}

impl<S: ShaderTypeList> Pipelines<S> {
    fn get_pipeline_handles<T: ShaderType>(&self) -> Option<Vec<ShaderHandle<T>>> {
        if let Some(pipelines) =
            self.write_pass
                .try_get::<GBufferWritePassPipeline<AttachmentsGBuffer, T::Material, T::Vertex>>()
        {
            Some(
                (0..pipelines.len())
                    .map(|index| ShaderHandle {
                        index,
                        _phantom: PhantomData,
                    })
                    .collect(),
            )
        } else {
            None
        }
    }
}

pub struct DeferredRenderer<S: ShaderTypeList> {
    frames: FramePool<DeferredRenderer<S>>,
    pipelines: Pipelines<S>,
    render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
    descriptors: DescriptorPool<GBufferDescriptorSet>,
    skybox: Skybox<GBufferSkyboxPipeline<AttachmentsGBuffer>>,
    mesh: MeshPack<CommonVertex>,
    g_buffer: GBuffer,
    swapchain: VulkanSwapchain<<Self as Frame>::Attachments>,
    current_frame: Option<FrameData<Self>>,
}

pub struct DeferredRendererFrameState<S: ShaderTypeList> {
    commands: Commands<S>,
    draw_graph: DrawGraph,
}

impl<T: ShaderTypeList> Frame for DeferredRenderer<T> {
    const REQUIRED_COMMANDS: usize = 3 + T::LEN;
    type Attachments = AttachmentsGBuffer;
    type State = DeferredRendererFrameState<T>;

    fn begin_frame(
        &mut self,
        device: &VulkanDevice,
        camera_matrices: &CameraMatrices,
    ) -> Result<(), Box<dyn Error>> {
        let (index, primary_command) = self.frames.primary_commands.next();
        let primary_command = device.begin_primary_command(primary_command)?;
        let swapchain_frame = device.get_frame(&self.swapchain, self.frames.image_sync[index])?;
        let camera_descriptor = self.frames.camera_uniform.descriptors[index];
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
        S: ShaderType,
        D: Drawable<Material = S::Material, Vertex = S::Vertex>,
        M: MaterialPackList,
        V: MeshPackList,
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
        device.present_frame(&self.swapchain, primary_command, swapchain_frame)?;
        Ok(())
    }

    fn get_shader_handles<S: ShaderType>(&self) -> Option<Vec<ShaderHandle<S>>> {
        self.pipelines.get_pipeline_handles()
    }
}

impl GBuffer {
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

fn adapt_shader_source_map<S: ShaderTypeList>(
    mut adapted: HashMap<TypeId, Vec<PathBuf>>,
    shaders: &S,
) -> HashMap<TypeId, Vec<PathBuf>> {
    if S::LEN > 0 {
        adapted.insert(
            TypeId::of::<
                GBufferWritePassPipeline<
                    AttachmentsGBuffer,
                    <S::Item as ShaderType>::Material,
                    <S::Item as ShaderType>::Vertex,
                >,
            >(),
            shaders
                .shaders()
                .iter()
                .map(|s| s.source().into())
                .collect(),
        );
        adapt_shader_source_map::<S::Next>(adapted, shaders.next())
    } else {
        adapted
    }
}

impl Context {
    pub fn create_deferred_renderer<B: ShaderTypeList>(
        &mut self,
        shaders: &B,
    ) -> Result<DeferredRenderer<B>, Box<dyn Error>> {
        let g_buffer = self.create_g_buffer()?;
        // TODO: Consider revamping the module hierarchy
        let swapchain = self.create_swapchain::<AttachmentsGBuffer>(
            &self.instance,
            &self.surface,
            |swapchain_image, extent| {
                self.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                    g_buffer.get_framebuffer_builder(swapchain_image),
                    extent,
                )
            },
        )?;
        let frames = self.create_frame_pool(&swapchain)?;
        let render_pass = self.get_render_pass()?;
        let pipelines = self.create_pipelines(shaders)?;
        let skybox = self.create_skybox(
            Path::new("assets/skybox/skybox"),
            ShaderDirectory::new(Path::new("shaders/spv/skybox")),
        )?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<GBufferDescriptorSet>::new(1).write_images::<InputAttachment, _>(
                &GBufferShadingPass::<AttachmentsGBuffer>::references()
                    .get_input_attachments(&swapchain.framebuffers[0]),
            ),
        )?;
        let mesh = self.load_mesh_pack(
            &[MeshBuilder::plane_subdivided(
                0,
                2.0 * Vector3::y(),
                2.0 * Vector3::x(),
                Vector3::zero(),
                false,
            )
            .offset(Vector3::new(-1.0, -1.0, 0.0))
            .build()],
            usize::MAX,
        )?;

        Ok(DeferredRenderer {
            frames,
            pipelines,
            render_pass,
            descriptors,
            mesh,
            skybox,
            swapchain,
            g_buffer,
            current_frame: None,
        })
    }

    pub fn destroy_deferred_renderer<S: ShaderTypeList>(&self, renderer: &mut DeferredRenderer<S>) {
        self.destroy_frame_pool(&mut renderer.frames);
        self.destroy_pipelines(&mut renderer.pipelines);
        self.destroy_descriptor_pool(&mut renderer.descriptors);
        self.destroy_mesh_pack(&mut renderer.mesh);
        self.destroy_skybox(&mut renderer.skybox);
        self.destroy_swapchain(&mut renderer.swapchain);
        self.destroy_g_buffer(&mut renderer.g_buffer);
    }

    fn create_pipelines<B: ShaderTypeList>(
        &self,
        shaders: &B,
    ) -> Result<Pipelines<B>, Box<dyn Error>> {
        let image_extent = self.physical_device.surface_properties.get_current_extent();
        let write_pass =
            self.create_pipeline_list::<B>(&adapt_shader_source_map::<B>(HashMap::new(), shaders))?;
        let depth_prepass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/deferred/depth_prepass")),
            image_extent,
        )?;
        let shading_pass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/deferred/gbuffer_combine")),
            image_extent,
        )?;
        Ok(Pipelines {
            write_pass,
            depth_prepass,
            shading_pass,
            _phantom: PhantomData,
        })
    }

    fn destroy_pipelines<S: ShaderTypeList>(&self, pipelines: &mut Pipelines<S>) {
        self.destory_pipeline_list(&mut pipelines.write_pass);
        self.destroy_pipeline(&mut pipelines.depth_prepass);
        self.destroy_pipeline(&mut pipelines.shading_pass);
    }

    pub fn create_g_buffer(&mut self) -> Result<GBuffer, Box<dyn Error>> {
        let combined = self.create_color_attachment_image()?;
        let albedo = self.create_color_attachment_image()?;
        let normal = self.create_color_attachment_image()?;
        let position = self.create_color_attachment_image()?;
        let depth = self.create_depth_stencil_attachment_image()?;
        Ok(GBuffer {
            combined,
            albedo,
            normal,
            position,
            depth,
        })
    }

    pub fn destroy_g_buffer(&self, g_buffer: &mut GBuffer) {
        self.destroy_image(&mut g_buffer.combined);
        self.destroy_image(&mut g_buffer.albedo);
        self.destroy_image(&mut g_buffer.normal);
        self.destroy_image(&mut g_buffer.position);
        self.destroy_image(&mut g_buffer.depth);
    }
}

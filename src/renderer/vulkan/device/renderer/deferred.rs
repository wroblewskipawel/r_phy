use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use ash::vk;

use crate::{
    math::types::{Matrix4, Vector3},
    renderer::{
        camera::CameraMatrices,
        model::{CommonVertex, Material, MaterialTypeList, MeshBuilder, Vertex},
        vulkan::device::{
            command::{
                level::{Primary, Secondary},
                operation::Graphics,
                BeginCommand, FinishedCommand, Persistent, PersistentCommandPool,
            },
            descriptor::{
                CameraDescriptorSet, Descriptor, DescriptorPool, DescriptorSetWriter,
                GBufferDescriptorSet,
            },
            frame::Frame,
            framebuffer::{
                presets::AttachmentsGBuffer, AttachmentReferences, AttachmentsBuilder, Builder,
                ClearColor, ClearDeptStencil, ClearNone, ClearValueBuilder, FramebufferHandle,
                InputAttachment,
            },
            image::VulkanImage2D,
            pipeline::{
                GBufferDepthPrepasPipeline, GBufferShadingPassPipeline, GBufferSkyboxPipeline,
                GBufferWritePassPipeline, GraphicsPipeline, GraphicsPipelineConfig,
                GraphicsPipelineTypeList, ModelMatrix, ModelNormalMatrix, PipelineList,
                ShaderDirectory,
            },
            render_pass::{
                DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferSkyboxPass,
                GBufferWritePass, RenderPass, Subpass,
            },
            resources::{
                MaterialPackList, MeshPack, MeshPackList, VulkanMaterialHandle, VulkanMeshHandle,
            },
            skybox::Skybox,
            swapchain::{self, SwapchainFrame},
            VulkanDevice,
        },
    },
};

impl<T: MaterialTypeList> GraphicsPipelineTypeList for T {
    const LEN: usize = T::LEN;
    type Pipeline = GBufferWritePassPipeline<AttachmentsGBuffer, T::Item>;
    type Next = T::Next;
}

pub struct GBuffer {
    pub combined: VulkanImage2D,
    pub albedo: VulkanImage2D,
    pub normal: VulkanImage2D,
    pub position: VulkanImage2D,
    pub depth: VulkanImage2D,
}

pub struct DeferredRenderer<M: MaterialTypeList> {
    render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
    depth_prepass: GraphicsPipeline<GBufferDepthPrepasPipeline<AttachmentsGBuffer>>,
    write_pass: PipelineList,
    shading_pass: GraphicsPipeline<GBufferShadingPassPipeline<AttachmentsGBuffer>>,
    descriptors: DescriptorPool<GBufferDescriptorSet>,
    skybox: Skybox<GBufferSkyboxPipeline<AttachmentsGBuffer>>,
    mesh: MeshPack<CommonVertex>,
    _phantom: PhantomData<M>,
}
struct StatefulCommand {
    command: BeginCommand<Persistent, Secondary, Graphics>,
    mesh_pack_index: Option<u32>,
}

impl VulkanDevice {
    fn create_stateful_command<S: Subpass<AttachmentsGBuffer>, C: GraphicsPipelineConfig>(
        &self,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
        framebuffer: FramebufferHandle<AttachmentsGBuffer>,
        pipeline: &GraphicsPipeline<C>,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
    ) -> Result<StatefulCommand, Box<dyn Error>> {
        let (_, command) = pool.next();
        let command = self.record_command(
            self.begin_secondary_command::<_, _, _, S>(command, render_pass, framebuffer)?,
            |command| {
                command
                    .bind_pipeline(pipeline)
                    .bind_descriptor_set(pipeline, camera_descriptor)
            },
        );
        Ok(StatefulCommand {
            command,
            mesh_pack_index: None,
        })
    }
}

struct WritePassMaterialCommands {
    commands: HashMap<TypeId, Option<StatefulCommand>>,
}

impl WritePassMaterialCommands {
    fn record<M: Material, F: FnOnce(StatefulCommand) -> StatefulCommand>(&mut self, f: F) {
        self.commands
            .entry(TypeId::of::<M>())
            .and_modify(|command| *command = Some(f(command.take().unwrap())));
    }
}

impl VulkanDevice {
    fn prepare_write_pass_command<N: MaterialTypeList>(
        &self,
        mut commands: HashMap<TypeId, Option<StatefulCommand>>,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
        swapchain_frame: &SwapchainFrame<AttachmentsGBuffer>,
        render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        pipelines: &PipelineList,
    ) -> Result<HashMap<TypeId, Option<StatefulCommand>>, Box<dyn Error>> {
        if N::LEN > 0 {
            let pipeline = pipelines
                .try_get::<GBufferWritePassPipeline<AttachmentsGBuffer, N::Item>>()
                .unwrap();
            commands.insert(
                TypeId::of::<N::Item>(),
                Some(
                    self.create_stateful_command::<GBufferWritePass<AttachmentsGBuffer>, _>(
                        pool,
                        render_pass,
                        swapchain_frame.framebuffer,
                        &pipeline,
                        camera_descriptor,
                    )?,
                ),
            );
            self.prepare_write_pass_command::<N::Next>(
                commands,
                camera_descriptor,
                swapchain_frame,
                render_pass,
                pool,
                pipelines,
            )
        } else {
            Ok(commands)
        }
    }

    fn create_write_pass_material_commands<M: MaterialTypeList>(
        &self,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
        swapchain_frame: &SwapchainFrame<AttachmentsGBuffer>,
        render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        pipelines: &PipelineList,
    ) -> Result<WritePassMaterialCommands, Box<dyn Error>> {
        Ok(WritePassMaterialCommands {
            commands: self.prepare_write_pass_command::<M>(
                HashMap::new(),
                camera_descriptor,
                swapchain_frame,
                render_pass,
                pool,
                pipelines,
            )?,
        })
    }
}

pub struct DeferredRendererFrameState<M: MaterialTypeList> {
    // TODO: These Command wrappers doesn't need to containt fence
    depth_prepass: StatefulCommand,
    write_pass: WritePassMaterialCommands,
    shading_pass: BeginCommand<Persistent, Secondary, Graphics>,
    skybox_pass: BeginCommand<Persistent, Secondary, Graphics>,
    _phantom: PhantomData<M>,
}

impl<M: MaterialTypeList> Frame for DeferredRenderer<M> {
    const REQUIRED_COMMANDS: usize = 3 + M::LEN;
    type Attachments = AttachmentsGBuffer;
    type State = DeferredRendererFrameState<M>;

    fn begin(
        &self,
        device: &VulkanDevice,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        swapchain_frame: &SwapchainFrame<Self::Attachments>,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
        camera_matrices: &CameraMatrices,
    ) -> Result<Self::State, Box<dyn Error>> {
        // TODO: Refactor!!!
        let depth_prepass = {
            let StatefulCommand {
                command,
                mesh_pack_index,
            } = device.create_stateful_command::<GBufferDepthPrepas<AttachmentsGBuffer>, _>(
                pool,
                self.render_pass,
                swapchain_frame.framebuffer,
                &self.depth_prepass,
                camera_descriptor,
            )?;
            let command = device.record_command(command, |command| {
                command
                    .bind_pipeline(&self.depth_prepass)
                    .bind_descriptor_set(&self.depth_prepass, camera_descriptor)
            });
            StatefulCommand {
                command,
                mesh_pack_index,
            }
        };
        let write_pass = device.create_write_pass_material_commands::<M>(
            camera_descriptor,
            swapchain_frame,
            self.render_pass,
            pool,
            &self.write_pass,
        )?;
        let (_, shading_pass) = pool.next();
        let shading_pass = device.begin_secondary_command::<_, _, _, GBufferShadingPass<_>>(
            shading_pass,
            self.render_pass,
            swapchain_frame.framebuffer,
        )?;
        let shading_pass = device.record_command(shading_pass, |command| {
            command
                .bind_pipeline(&self.shading_pass)
                .bind_descriptor_set(&self.shading_pass, self.descriptors[0])
                .bind_mesh_pack(&self.mesh)
                .draw_mesh(self.mesh[0])
        });
        let (_, skybox_pass) = pool.next();
        let skybox_pass = device.begin_secondary_command::<_, _, _, GBufferSkyboxPass<_>>(
            skybox_pass,
            self.render_pass,
            swapchain_frame.framebuffer,
        )?;
        let skybox_pass = device.record_command(skybox_pass, |command| {
            command.draw_skybox(&self.skybox, *camera_matrices)
        });
        Ok(DeferredRendererFrameState {
            depth_prepass,
            write_pass,
            shading_pass,
            skybox_pass,
            _phantom: PhantomData,
        })
    }

    fn draw_mesh<V: Vertex, T: Material>(
        &self,
        state: Self::State,
        device: &VulkanDevice,
        model: &Matrix4,
        mesh: VulkanMeshHandle<V>,
        material: VulkanMaterialHandle<T>,
        mesh_packs: &impl MeshPackList,
        material_packs: &impl MaterialPackList,
    ) -> Self::State {
        let VulkanMeshHandle {
            mesh_pack_index,
            mesh_index,
            ..
        } = mesh;
        let VulkanMaterialHandle { material_index, .. } = material;
        let Self::State {
            depth_prepass,
            mut write_pass,
            shading_pass,
            skybox_pass,
            ..
        } = state;

        let meshes = mesh_packs.try_get::<V>().unwrap();
        let mesh_ranges = meshes.get(mesh_index as usize);
        let materials = material_packs.try_get::<T>().unwrap();
        let material = materials.get_descriptor(material_index as usize);
        let depth_prepass = {
            let StatefulCommand {
                command,
                mesh_pack_index: current_mesh_pack_index,
            } = depth_prepass;
            let command = device.record_command(command, |command| {
                if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                    command.bind_mesh_pack(meshes)
                } else {
                    command
                }
                .push_constants::<_, ModelMatrix>(&self.depth_prepass, &(model.into()))
                .draw_mesh(mesh_ranges)
            });
            StatefulCommand {
                command,
                mesh_pack_index: Some(mesh_pack_index),
            }
        };
        let write_pass_pipeline = self
            .write_pass
            .try_get::<GBufferWritePassPipeline<AttachmentsGBuffer, T>>()
            .unwrap();
        write_pass.record::<T, _>(
            |StatefulCommand {
                 command,
                 mesh_pack_index: current_mesh_pack_index,
             }| {
                let command = device.record_command(command, |command| {
                    if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                        command.bind_mesh_pack(meshes)
                    } else {
                        command
                    }
                    .bind_descriptor_set(&write_pass_pipeline, material)
                    .push_constants::<_, ModelNormalMatrix>(&write_pass_pipeline, &(model.into()))
                    .draw_mesh(mesh_ranges)
                });
                StatefulCommand {
                    command,
                    mesh_pack_index: Some(mesh_pack_index),
                }
            },
        );
        Self::State {
            depth_prepass,
            write_pass,
            shading_pass,
            skybox_pass,
            _phantom: PhantomData,
        }
    }

    fn end(
        &self,
        state: Self::State,
        device: &VulkanDevice,
        swapchain_frame: &SwapchainFrame<Self::Attachments>,
        primary_command: BeginCommand<Persistent, Primary, Graphics>,
    ) -> Result<FinishedCommand<Persistent, Primary, Graphics>, Box<dyn Error>> {
        let Self::State {
            depth_prepass,
            skybox_pass,
            write_pass,
            shading_pass,
            ..
        } = state;
        let depth_prepass = device.finish_command(depth_prepass.command)?;
        let skybox_pass = device.finish_command(skybox_pass)?;
        let write_pass = write_pass
            .commands
            .into_iter()
            .flat_map(|(_, mut command)| device.finish_command(command.take().unwrap().command))
            .collect::<Vec<_>>();
        let shading_pass = device.finish_command(shading_pass)?;

        let clear_values = ClearValueBuilder::new()
            .push(ClearNone {})
            .push(ClearDeptStencil {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            })
            .push(ClearColor {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })
            .push(ClearColor {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })
            .push(ClearColor {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })
            .push(ClearColor {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            });
        let primary_command = device.record_command(primary_command, |command| {
            let command = command
                .begin_render_pass(swapchain_frame, &self.render_pass, &clear_values)
                .write_secondary(&depth_prepass)
                .next_render_pass()
                .write_secondary(&skybox_pass)
                .next_render_pass();
            write_pass
                .into_iter()
                .fold(command, |command, write_pass| {
                    command.write_secondary(&write_pass)
                })
                .next_render_pass()
                .write_secondary(&shading_pass)
                .end_render_pass()
        });
        let primary_command = device.finish_command(primary_command)?;
        Ok(primary_command)
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

fn adapt_shader_source_map<M: MaterialTypeList>(
    mut adapted: HashMap<TypeId, PathBuf>,
    shaders: &HashMap<TypeId, PathBuf>,
) -> HashMap<TypeId, PathBuf> {
    if M::LEN > 0 {
        adapted.insert(
            TypeId::of::<GBufferWritePassPipeline<AttachmentsGBuffer, M::Item>>(),
            shaders.get(&TypeId::of::<M::Item>()).unwrap().clone(),
        );
        adapt_shader_source_map::<M::Next>(adapted, shaders)
    } else {
        adapted
    }
}

impl VulkanDevice {
    pub fn create_deferred_renderer<M: MaterialTypeList>(
        &self,
        swapchain: &swapchain::VulkanSwapchain<AttachmentsGBuffer>,
        shaders: &HashMap<TypeId, PathBuf>,
    ) -> Result<DeferredRenderer<M>, Box<dyn Error>> {
        let render_pass = self.get_render_pass()?;
        let image_extent = self.physical_device.surface_properties.get_current_extent();
        let depth_prepass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/deferred/depth_prepass")),
            image_extent,
        )?;
        let write_pass =
            self.create_pipeline_list::<M>(&adapt_shader_source_map::<M>(HashMap::new(), shaders))?;
        let shading_pass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/deferred/gbuffer_combine")),
            image_extent,
        )?;
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
            render_pass,
            depth_prepass,
            write_pass,
            shading_pass,
            descriptors,
            mesh,
            skybox,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_deferred_renderer<M: MaterialTypeList>(
        &self,
        renderer: &mut DeferredRenderer<M>,
    ) {
        self.destroy_pipeline(&mut renderer.depth_prepass);
        self.destory_pipeline_list(&mut renderer.write_pass);
        self.destroy_pipeline(&mut renderer.shading_pass);
        self.destroy_descriptor_pool(&mut renderer.descriptors);
        self.destroy_mesh_pack(&mut renderer.mesh);
        self.destroy_skybox(&mut renderer.skybox);
    }

    pub fn create_g_buffer(&self) -> Result<GBuffer, Box<dyn Error>> {
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

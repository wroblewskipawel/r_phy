use std::{error::Error, path::Path};

use ash::vk;

use crate::{
    math::types::{Matrix4, Vector3},
    renderer::{
        camera::CameraMatrices,
        model::MeshBuilder,
        vulkan::{
            device::{
                command::{
                    level::{Primary, Secondary},
                    operation::Graphics,
                    BeginCommand, FinishedCommand, Persistent, PersistentCommandPool,
                },
                descriptor::{
                    CameraDescriptorSet, Descriptor, DescriptorPool, GBufferDescriptorSet,
                },
                frame::Frame,
                framebuffer::{
                    presets::AttachmentsGBuffer, AttachmentReferences, AttachmentsBuilder, Builder,
                    ClearColor, ClearDeptStencil, ClearNone, ClearValueBuilder,
                },
                image::VulkanImage2D,
                material::MaterialPack,
                mesh::MeshPack,
                pipeline::{
                    GBufferDepthPrepasPipeline, GBufferShadingPassPipeline, GBufferSkyboxPipeline,
                    GBufferWritePassPipeline, GraphicsPipeline, ModelMatrix, ModelNormalMatrix,
                    ShaderDirectory,
                },
                render_pass::{
                    DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferSkyboxPass,
                    GBufferWritePass, RenderPass, Subpass,
                },
                skybox::Skybox,
                swapchain::{self, SwapchainFrame},
                VulkanDevice,
            },
            VulkanMaterialHandle, VulkanMeshHandle,
        },
    },
};

struct GBuffer {
    pub combined: VulkanImage2D,
    pub albedo: VulkanImage2D,
    pub normal: VulkanImage2D,
    pub position: VulkanImage2D,
    pub depth: VulkanImage2D,
}

pub struct DeferredRenderer {
    g_buffer: GBuffer,
    render_pass: RenderPass<DeferedRenderPass<AttachmentsGBuffer>>,
    depth_prepass: GraphicsPipeline<GBufferDepthPrepasPipeline<AttachmentsGBuffer>>,
    write_pass: GraphicsPipeline<GBufferWritePassPipeline<AttachmentsGBuffer>>,
    shading_pass: GraphicsPipeline<GBufferShadingPassPipeline<AttachmentsGBuffer>>,
    descriptors: DescriptorPool<GBufferDescriptorSet>,
    skybox: Skybox<GBufferSkyboxPipeline<AttachmentsGBuffer>>,
    mesh: MeshPack,
}

pub struct DeferredRendererFrameState {
    // TODO: These Command wrappers doesn't need to containt fence
    pub depth_prepass: BeginCommand<Persistent, Secondary, Graphics>,
    pub write_pass: BeginCommand<Persistent, Secondary, Graphics>,
    pub shading_pass: BeginCommand<Persistent, Secondary, Graphics>,
    pub skybox_pass: BeginCommand<Persistent, Secondary, Graphics>,
    pub current_mesh_pack_index: Option<u32>,
}

impl Frame for DeferredRenderer {
    const REQUIRED_COMMANDS: usize = 4;
    type Attachments = AttachmentsGBuffer;
    type State = DeferredRendererFrameState;

    fn begin(
        &self,
        device: &VulkanDevice,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        swapchain_frame: &SwapchainFrame<Self::Attachments>,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
        camera_matrices: &CameraMatrices,
    ) -> Result<Self::State, Box<dyn Error>> {
        let (_, depth_prepass) = pool.next();
        let depth_prepass = device.begin_secondary_command::<_, _, _, GBufferDepthPrepas<_>>(
            depth_prepass,
            self.render_pass,
            swapchain_frame.framebuffer,
        )?;
        let depth_prepass = device.record_command(depth_prepass, |command| {
            command
                .bind_pipeline(&self.depth_prepass)
                .bind_descriptor_set(&self.depth_prepass, camera_descriptor)
        });
        let (_, write_pass) = pool.next();
        let write_pass = device.begin_secondary_command::<_, _, _, GBufferWritePass<_>>(
            write_pass,
            self.render_pass,
            swapchain_frame.framebuffer,
        )?;
        let write_pass = device.record_command(write_pass, |command| {
            command
                .bind_pipeline(&self.write_pass)
                .bind_descriptor_set(&self.write_pass, camera_descriptor)
        });
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
                .draw_mesh(self.mesh.meshes[0])
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
            current_mesh_pack_index: None,
        })
    }

    fn draw_mesh(
        &self,
        state: Self::State,
        device: &VulkanDevice,
        model: &Matrix4,
        mesh: VulkanMeshHandle,
        material: VulkanMaterialHandle,
        mesh_packs: &[MeshPack],
        material_packs: &[MaterialPack],
    ) -> Self::State {
        let VulkanMeshHandle {
            mesh_pack_index,
            mesh_index,
        } = mesh;
        let VulkanMaterialHandle {
            material_pack_index,
            material_index,
        } = material;
        let Self::State {
            depth_prepass,
            write_pass,
            shading_pass,
            skybox_pass,
            current_mesh_pack_index,
        } = state;

        let meshes = &mesh_packs[mesh_pack_index as usize];
        let material =
            material_packs[material_pack_index as usize].descriptors[material_index as usize];
        let mesh_ranges = meshes.meshes[mesh_index as usize];
        let depth_prepass = device.record_command(depth_prepass, |command| {
            if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                command.bind_mesh_pack(meshes)
            } else {
                command
            }
            .push_constants::<_, ModelMatrix>(&self.depth_prepass, &(model.into()))
            .draw_mesh(mesh_ranges)
        });
        let write_pass = device.record_command(write_pass, |command| {
            if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                command.bind_mesh_pack(meshes)
            } else {
                command
            }
            .bind_descriptor_set(&self.write_pass, material)
            .push_constants::<_, ModelNormalMatrix>(&self.write_pass, &(model.into()))
            .draw_mesh(mesh_ranges)
        });
        Self::State {
            depth_prepass,
            write_pass,
            shading_pass,
            skybox_pass,
            current_mesh_pack_index: Some(mesh_pack_index),
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
        let depth_prepass = device.finish_command(depth_prepass)?;
        let skybox_pass = device.finish_command(skybox_pass)?;
        let write_pass = device.finish_command(write_pass)?;
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
            command
                .begin_render_pass(&swapchain_frame, &self.render_pass, &clear_values)
                .write_secondary(&depth_prepass)
                .next_render_pass()
                .write_secondary(&skybox_pass)
                .next_render_pass()
                .write_secondary(&write_pass)
                .next_render_pass()
                .write_secondary(&shading_pass)
                .end_render_pass()
        });
        let primary_command = device.finish_command(primary_command)?;
        Ok(primary_command)
    }
}

impl DeferredRenderer {
    pub fn get_framebuffer_builder(
        &self,
        swapchain_image: vk::ImageView,
    ) -> Builder<AttachmentsGBuffer> {
        AttachmentsBuilder::new()
            .push(swapchain_image)
            .push(self.g_buffer.depth.image_view)
            .push(self.g_buffer.position.image_view)
            .push(self.g_buffer.normal.image_view)
            .push(self.g_buffer.albedo.image_view)
            .push(self.g_buffer.combined.image_view)
    }
}

impl VulkanDevice {
    pub fn create_deferred_renderer(&self) -> Result<DeferredRenderer, Box<dyn Error>> {
        let g_buffer = self.create_g_buffer()?;
        let render_pass = self.get_render_pass()?;
        let image_extent = self.physical_device.surface_properties.get_current_extent();
        let depth_prepass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/depth_prepass")),
            image_extent,
        )?;
        let write_pass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/gbuffer_write")),
            image_extent,
        )?;
        let shading_pass = self.create_graphics_pipeline(
            ShaderDirectory::new(Path::new("shaders/spv/gbuffer_combine")),
            image_extent,
        )?;
        let skybox = self.create_skybox(
            Path::new("assets/skybox/skybox"),
            ShaderDirectory::new(Path::new("shaders/spv/skybox")),
        )?;
        let descriptors = self.create_descriptor_pool(GBufferDescriptorSet::builder(), 1)?;
        let mesh = self.load_mesh_pack(&[MeshBuilder::plane_subdivided(
            0,
            2.0 * Vector3::y(),
            2.0 * Vector3::x(),
            Vector3::zero(),
            false,
        )
        .offset(Vector3::new(-1.0, -1.0, 0.0))
        .build()])?;

        Ok(DeferredRenderer {
            g_buffer,
            render_pass,
            depth_prepass,
            write_pass,
            shading_pass,
            descriptors,
            mesh,
            skybox,
        })
    }

    pub fn update_deferred_renderer_input_descriptors(
        &self,
        renderer: &mut DeferredRenderer,
        swapchain: &swapchain::VulkanSwapchain<AttachmentsGBuffer>,
    ) {
        let descriptor_write = renderer.descriptors.get_writer().write_image(
            &GBufferShadingPass::<AttachmentsGBuffer>::references()
                .get_input_attachments(&swapchain.framebuffers[0]),
        );
        self.write_descriptor_sets(&mut renderer.descriptors, descriptor_write);
    }

    fn create_g_buffer(&self) -> Result<GBuffer, Box<dyn Error>> {
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

    pub fn destroy_deferred_renderer(&self, renderer: &mut DeferredRenderer) {
        self.destroy_g_buffer(&mut renderer.g_buffer);
        self.destroy_graphics_pipeline(&mut renderer.depth_prepass);
        self.destroy_graphics_pipeline(&mut renderer.write_pass);
        self.destroy_graphics_pipeline(&mut renderer.shading_pass);
        self.destroy_descriptor_pool(&mut renderer.descriptors);
        self.destroy_mesh_pack(&mut renderer.mesh);
        self.destroy_skybox(&mut renderer.skybox);
    }
}

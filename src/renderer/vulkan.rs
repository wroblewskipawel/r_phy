mod debug;
mod device;
mod surface;

use crate::math::types::Matrix4;

use self::device::{
    command::{
        level::{Primary, Secondary},
        operation::Graphics,
        BeginCommand, Persistent,
    },
    framebuffer::{ClearColor, ClearDeptStencil, ClearNone, ClearValueBuilder},
    material::MaterialPack,
    mesh::MeshPack,
    pipeline::{
        GraphicsPipeline, GraphicsPipelineColorDepthCombinedTextured, GraphicsPipelineColorPass,
        GraphicsPipelineForwardDepthPrepass, ModelMatrix, ShaderDirectory,
    },
    render_pass::{
        ColorDepthCombinedRenderPass, ColorPassSubpass, DepthPrepassSubpass,
        ForwardDepthPrepassRenderPass,
    },
    skybox::Skybox,
};

use super::{
    camera::{Camera, CameraMatrices},
    model::{Material, MaterialHandle, Mesh, MeshHandle, Model},
    Renderer,
};
use ash::{vk, Entry, Instance};
use debug::VulkanDebugUtils;
use device::{
    render_pass::RenderPass,
    swapchain::{SwapchainFrame, VulkanSwapchain},
    VulkanDevice,
};
use std::{
    error::Error,
    ffi::{c_char, CStr},
    path::Path,
};
use surface::VulkanSurface;
use winit::window::Window;

struct FrameState {
    camera_matrices: CameraMatrices,
    primary_command: BeginCommand<Persistent, Primary, Graphics>,
    depth_prepass_command: BeginCommand<Persistent, Secondary, Graphics>,
    color_pass_command: BeginCommand<Persistent, Secondary, Graphics>,
    swapchain_frame: SwapchainFrame,
    mesh_pack_index: Option<u32>,
}

struct DepthPrepassPipelie {
    render_pass: RenderPass<ForwardDepthPrepassRenderPass>,
    depth_prepass: GraphicsPipeline<GraphicsPipelineForwardDepthPrepass>,
    color_pass: GraphicsPipeline<GraphicsPipelineColorPass>,
}

pub(super) struct VulkanRenderer {
    depth_prepass_pipeline: DepthPrepassPipelie,
    current_frame_state: Option<FrameState>,
    materials: Vec<MaterialPack>,
    meshes: Vec<MeshPack>,
    skybox: Skybox,
    pipeline: GraphicsPipeline<GraphicsPipelineColorDepthCombinedTextured>,
    swapchain: VulkanSwapchain,
    render_pass: RenderPass<ColorDepthCombinedRenderPass>,
    device: VulkanDevice,
    surface: VulkanSurface,
    debug_utils: Option<VulkanDebugUtils>,
    instance: Instance,
    _entry: Entry,
}

fn check_required_extension_support(
    entry: &Entry,
    mut extension_names: impl Iterator<Item = &'static CStr>,
) -> Result<Vec<*const c_char>, Box<dyn Error>> {
    let supported_extensions = entry.enumerate_instance_extension_properties(None)?;
    let supported = extension_names.try_fold(Vec::new(), |mut supported, req| {
        supported_extensions
            .iter()
            .any(|sup| unsafe { CStr::from_ptr(&sup.extension_name as *const _) } == req)
            .then(|| {
                supported.push(req.as_ptr());
                supported
            })
            .ok_or(format!(
                "Required extension {} not supported!",
                req.to_string_lossy()
            ))
    })?;
    Ok(supported)
}

fn check_required_layer_support(
    entry: &Entry,
    mut layer_names: impl Iterator<Item = &'static CStr>,
) -> Result<Vec<*const c_char>, Box<dyn Error>> {
    let supported_layers = entry.enumerate_instance_layer_properties()?;
    let supported = layer_names.try_fold(Vec::new(), |mut supported, req| {
        supported_layers
            .iter()
            .any(|sup| unsafe { CStr::from_ptr(&sup.layer_name as *const _) } == req)
            .then(|| {
                supported.push(req.as_ptr());
                supported
            })
            .ok_or(format!(
                "Required layer {} not supported!",
                req.to_string_lossy()
            ))
    })?;
    Ok(supported)
}

impl VulkanRenderer {
    pub fn new(window: &Window) -> Result<Self, Box<dyn Error>> {
        let entry = unsafe { Entry::load()? };
        let enabled_layer_names = check_required_layer_support(
            &entry,
            VulkanDebugUtils::required_layers().iter().copied(),
        )?;
        let enabled_extension_names = check_required_extension_support(
            &entry,
            VulkanDebugUtils::required_extensions()
                .iter()
                .chain(VulkanSurface::required_extensions())
                .copied(),
        )?;
        let application_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_1,
            ..Default::default()
        };
        let mut debug_messenger_info = VulkanDebugUtils::create_info();
        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&enabled_layer_names)
            .enabled_extension_names(&enabled_extension_names)
            .push_next(&mut debug_messenger_info);
        let instance = unsafe { entry.create_instance(&create_info, None)? };
        let debug_utils = VulkanDebugUtils::build(&entry, &instance)?;
        let surface = VulkanSurface::create(&entry, &instance, window)?;
        let device = VulkanDevice::create(&instance, &surface)?;
        let render_pass = device.get_render_pass::<ColorDepthCombinedRenderPass>()?;
        let swapchain =
            device.create_swapchain::<ForwardDepthPrepassRenderPass>(&instance, &surface)?;
        // TODO: Error handling should be improved - currently when shader source files are missing,
        // execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
        // while error message indicating true cause of the issue is never presented to the user
        // TODO: User should be able to load custom shareds,
        // while also some preset of preconfigured one should be available
        // API for user-defined shaders should be based on PipelineLayoutBuilder type-list
        let pipeline = device
            .create_graphics_pipeline::<GraphicsPipelineColorDepthCombinedTextured>(
                ShaderDirectory::new(Path::new("shaders/spv/unlit_textured")),
                swapchain.image_extent,
            )?;
        let skybox = device.create_skybox(&swapchain, Path::new("assets/skybox/skybox"))?;

        let depth_prepass_pipeline = DepthPrepassPipelie {
            render_pass: device.get_render_pass()?,
            depth_prepass: device.create_graphics_pipeline(
                ShaderDirectory::new(Path::new("shaders/spv/depth_prepass")),
                swapchain.image_extent,
            )?,
            color_pass: device.create_graphics_pipeline(
                ShaderDirectory::new(Path::new("shaders/spv/unlit_textured")),
                swapchain.image_extent,
            )?,
        };
        Ok(Self {
            depth_prepass_pipeline,
            current_frame_state: None,
            materials: vec![],
            meshes: vec![],
            skybox,
            pipeline,
            swapchain,
            render_pass,
            device,
            surface,
            debug_utils: Some(debug_utils),
            instance,
            _entry: entry,
        })
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        let _ = self.device.wait_idle();
        unsafe {
            self.materials
                .iter_mut()
                .for_each(|pack| self.device.destroy_material_pack(pack));
            self.meshes
                .iter_mut()
                .for_each(|pack| self.device.destroy_mesh_pack(pack));
            self.device.destroy_skybox(&mut self.skybox);
            self.device.destroy_graphics_pipeline(&mut self.pipeline);
            self.device
                .destroy_graphics_pipeline(&mut self.depth_prepass_pipeline.depth_prepass);
            self.device
                .destroy_graphics_pipeline(&mut self.depth_prepass_pipeline.color_pass);
            self.device.destroy_swapchain(&mut self.swapchain);
            self.device.destroy_render_passes();
            self.device.destroy_pipeline_layouts();
            self.device.destroy_descriptor_set_layouts();
            self.device.destroy();
            self.surface.destroy();
            drop(self.debug_utils.take());
            self.instance.destroy_instance(None);
        }
    }
}

struct VulkanMeshHandle {
    mesh_pack_index: u32,
    mesh_index: u32,
}

impl From<MeshHandle> for VulkanMeshHandle {
    fn from(value: MeshHandle) -> Self {
        Self {
            mesh_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            mesh_index: (0x00000000FFFFFFFF & value.0) as u32,
        }
    }
}

impl From<VulkanMeshHandle> for MeshHandle {
    fn from(value: VulkanMeshHandle) -> Self {
        Self(((value.mesh_pack_index as u64) << 32) + value.mesh_index as u64)
    }
}

struct VulkanMaterialHandle {
    material_pack_index: u32,
    material_index: u32,
}

impl From<MaterialHandle> for VulkanMaterialHandle {
    fn from(value: MaterialHandle) -> Self {
        Self {
            material_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            material_index: (0x00000000FFFFFFFF & value.0) as u32,
        }
    }
}

impl From<VulkanMaterialHandle> for MaterialHandle {
    fn from(value: VulkanMaterialHandle) -> Self {
        Self(((value.material_pack_index as u64) << 32) + value.material_index as u64)
    }
}

impl Renderer for VulkanRenderer {
    fn begin_frame(&mut self, camera: &dyn Camera) -> Result<(), Box<dyn Error>> {
        let camera_matrices = camera.get_matrices();
        let (primary_command, depth_prepass_command, color_pass_command, swapchain_frame) = self
            .device
            .begin_frame(&mut self.swapchain, &camera_matrices)?;
        let depth_prepass_command = self
            .device
            .begin_secondary_command::<_, _, _, DepthPrepassSubpass>(
                depth_prepass_command,
                self.depth_prepass_pipeline.render_pass,
                swapchain_frame.framebuffer,
            )?;
        let depth_prepass_command = self
            .device
            .record_command(depth_prepass_command, |command| {
                command
                    .bind_pipeline(&self.depth_prepass_pipeline.depth_prepass)
                    .bind_descriptor_set(&self.pipeline, swapchain_frame.camera_descriptor)
            });
        let color_pass_command = self
            .device
            .begin_secondary_command::<_, _, _, ColorPassSubpass>(
                color_pass_command,
                self.depth_prepass_pipeline.render_pass,
                swapchain_frame.framebuffer,
            )?;
        let color_pass_command = self.device.record_command(color_pass_command, |command| {
            command
                .bind_pipeline(&self.depth_prepass_pipeline.color_pass)
                .bind_descriptor_set(&self.pipeline, swapchain_frame.camera_descriptor)
        });
        self.current_frame_state.replace(FrameState {
            camera_matrices,
            primary_command,
            depth_prepass_command,
            color_pass_command,
            swapchain_frame,
            mesh_pack_index: None,
        });
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let FrameState {
            camera_matrices,
            primary_command,
            depth_prepass_command,
            color_pass_command,
            swapchain_frame,
            ..
        } = self
            .current_frame_state
            .take()
            .ok_or("current_frame is None!")?;
        let depth_prepass_command = self.device.finish_command(depth_prepass_command)?;
        let color_pass_command = self.device.record_command(color_pass_command, |command| {
            command.draw_skybox(&self.skybox, camera_matrices)
        });
        let color_pass_command = self.device.finish_command(color_pass_command)?;

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
            });
        let primary_command = self.device.record_command(primary_command, |command| {
            command
                .begin_render_pass(
                    &swapchain_frame,
                    &self.depth_prepass_pipeline.render_pass,
                    &clear_values,
                )
                .write_secondary(&depth_prepass_command)
                .next_render_pass()
                .write_secondary(&color_pass_command)
                .end_render_pass()
        });

        self.device
            .end_frame(&mut self.swapchain, primary_command, swapchain_frame)?;
        Ok(())
    }

    fn load_meshes(&mut self, meshes: &[Mesh]) -> Result<Vec<MeshHandle>, Box<dyn Error>> {
        let mesh_pack_index = self.meshes.len() as u32;
        self.meshes.push(self.device.load_mesh_pack(meshes)?);
        let pack = self.meshes.last().unwrap();
        Ok((0..pack.meshes.len() as u32)
            .map(|mesh_index| {
                VulkanMeshHandle {
                    mesh_pack_index,
                    mesh_index,
                }
                .into()
            })
            .collect())
    }

    fn load_materials(
        &mut self,
        materials: &[Material],
    ) -> Result<Vec<super::model::MaterialHandle>, Box<dyn Error>> {
        let material_pack_index = self.materials.len() as u32;
        self.materials
            .push(self.device.load_material_pack(materials)?);
        let pack = self.materials.last().unwrap();
        Ok((0..pack.descriptors.count as u32)
            .map(|material_index| {
                VulkanMaterialHandle {
                    material_pack_index,
                    material_index,
                }
                .into()
            })
            .collect())
    }

    fn draw(&mut self, model: Model, transform: &Matrix4) -> Result<(), Box<dyn Error>> {
        let Model { mesh, material } = model;
        let model_matrix: ModelMatrix = (*transform).into();
        let VulkanMeshHandle {
            mesh_pack_index,
            mesh_index,
        } = mesh.into();
        let VulkanMaterialHandle {
            material_pack_index,
            material_index,
        } = material.into();
        let FrameState {
            camera_matrices,
            primary_command,
            depth_prepass_command,
            color_pass_command,
            swapchain_frame,
            mesh_pack_index: current_mesh_pack_index,
        } = self
            .current_frame_state
            .take()
            .ok_or("current_frame is None!")?;
        let meshes = &self.meshes[mesh_pack_index as usize];
        let mesh_ranges = meshes.meshes[mesh_index as usize];
        let depth_prepass_command = self
            .device
            .record_command(depth_prepass_command, |command| {
                if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                    command.bind_mesh_pack(&self.meshes[mesh_pack_index as usize])
                } else {
                    command
                }
                .push_constants(&self.pipeline, &model_matrix)
                .draw_mesh(mesh_ranges)
            });
        let color_pass_command = self.device.record_command(color_pass_command, |command| {
            if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                command.bind_mesh_pack(&self.meshes[mesh_pack_index as usize])
            } else {
                command
            }
            .bind_descriptor_set(
                &self.pipeline,
                self.materials[material_pack_index as usize].descriptors[material_index as usize],
            )
            .push_constants(&self.pipeline, &model_matrix)
            .draw_mesh(mesh_ranges)
        });
        self.current_frame_state.replace(FrameState {
            camera_matrices,
            primary_command,
            depth_prepass_command,
            color_pass_command,
            swapchain_frame,
            mesh_pack_index: Some(mesh_pack_index),
        });
        Ok(())
    }
}

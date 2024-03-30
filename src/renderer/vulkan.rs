mod debug;
mod device;
mod surface;

use crate::{math::types::Matrix4, physics::shape};

use self::device::{
    command::{operation::Graphics, BeginCommand, Persistent},
    image::Texture2D,
    material::MaterialPack,
    mesh::MeshPack,
    pipeline::{DescriptorLayoutBuilder, GraphicsPipeline, GraphicsPipelineLayoutTextured},
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
    render_pass::VulkanRenderPass,
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
    command: BeginCommand<Persistent, Graphics>,
    swapchain_frame: SwapchainFrame,
    mesh_pack_index: Option<u32>,
}

pub(super) struct VulkanRenderer {
    current_frame_state: Option<FrameState>,
    materials: Vec<MaterialPack>,
    meshes: Vec<MeshPack>,
    skybox: Skybox,
    pipeline: GraphicsPipeline<GraphicsPipelineLayoutTextured>,
    swapchain: VulkanSwapchain,
    render_pass: VulkanRenderPass,
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
        let render_pass = device.create_render_pass()?;
        let swapchain = device.create_swapchain(&instance, &surface, &render_pass)?;
        let pipeline_layout = device.create_graphics_pipeline_layout(
            DescriptorLayoutBuilder::new()
                .push::<CameraMatrices>()
                .push::<Texture2D>(),
        )?;
        // TODO: Error handling should be improved - currently when shader source files are missing,
        // execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
        // while error message indicating true cause of the issue is never presented to the user
        // TODO: User should be able to load custom shareds,
        // while also some preset of preconfigured one should be available
        // API for user-defined shaders should be based on PipelineLayoutBuilder type-list
        let pipeline = device.create_graphics_pipeline(
            pipeline_layout,
            &render_pass,
            &swapchain,
            &[
                Path::new("shaders/spv/unlit_textured/vert.spv"),
                Path::new("shaders/spv/unlit_textured/frag.spv"),
            ],
            false,
        )?;
        let skybox =
            device.create_skybox(&render_pass, &swapchain, &Path::new("assets/skybox/skybox"))?;
        Ok(Self {
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
            self.device.destroy_swapchain(&mut self.swapchain);
            self.device.destroy_render_pass(&mut self.render_pass);
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
        let (command, swapchain_frame) = self
            .device
            .begin_frame(&mut self.swapchain, &camera_matrices)?;
        let command = self.device.record_command(command, |command| {
            command
                .begin_render_pass(&swapchain_frame, &self.render_pass)
                .bind_pipeline(&self.skybox.pipeline)
                // Camera descriptor set shoould be bound in VulkanSwapchain::begin_frame,
                // or VulkanSwapchain should not manage Camera uniform buffers
                .bind_camera_uniform_buffer(&self.skybox.pipeline, &swapchain_frame)
                .draw_skybox(&self.skybox, &swapchain_frame, camera)
                .bind_pipeline(&self.pipeline)
                .bind_camera_uniform_buffer(&self.skybox.pipeline, &swapchain_frame)
        });
        self.current_frame_state.replace(FrameState {
            command,
            swapchain_frame,
            mesh_pack_index: None,
        });
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let FrameState {
            command,
            swapchain_frame,
            ..
        } = self
            .current_frame_state
            .take()
            .ok_or("current_frame is None!")?;
        let command = self
            .device
            .record_command(command, |command| command.end_render_pass());
        self.device
            .end_frame(&mut self.swapchain, command, swapchain_frame)?;
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
        let VulkanMeshHandle {
            mesh_pack_index,
            mesh_index,
        } = mesh.into();
        let VulkanMaterialHandle {
            material_pack_index,
            material_index,
        } = material.into();
        let FrameState {
            command,
            swapchain_frame,
            mesh_pack_index: current_mesh_pack_index,
        } = self
            .current_frame_state
            .take()
            .ok_or("current_frame is None!")?;
        let meshes = &self.meshes[mesh_pack_index as usize];
        let mesh_ranges = meshes.meshes[mesh_index as usize];
        let command = self.device.record_command(command, |command| {
            if !current_mesh_pack_index.is_some_and(|index| index == mesh_pack_index) {
                command.bind_mesh_pack(&self.meshes[mesh_pack_index as usize])
            } else {
                command
            }
            .bind_material(
                &self.pipeline,
                &self.materials[material_pack_index as usize],
                material_index as usize,
            )
            .push_constants(&self.pipeline, vk::ShaderStageFlags::VERTEX, 0, transform)
            .draw_mesh(mesh_ranges)
        });
        self.current_frame_state.replace(FrameState {
            command,
            swapchain_frame,
            mesh_pack_index: Some(mesh_pack_index),
        });
        Ok(())
    }
}

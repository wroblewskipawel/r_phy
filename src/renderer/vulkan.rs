mod debug;
mod device;
mod surface;

use crate::math::types::Matrix4;

use self::device::{
    command::{operation::Graphics, BeginCommand, Persistent},
    pipeline::{DescriptorLayoutBuilder, GraphicsPipeline, GraphicsPipelineLayoutSimple},
    resources::MeshPack,
};

use super::{
    camera::CameraMatrices,
    mesh::{Mesh, MeshHandle},
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
    result::Result,
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
    meshes: Vec<MeshPack>,
    pipeline: GraphicsPipeline<GraphicsPipelineLayoutSimple>,
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
            DescriptorLayoutBuilder::new().push::<CameraMatrices>(),
        )?;
        let pipeline = device.create_graphics_pipeline(
            pipeline_layout,
            &render_pass,
            &swapchain,
            &[
                Path::new("shaders/spv/unlit/vert.spv"),
                Path::new("shaders/spv/unlit/frag.spv"),
            ],
        )?;
        Ok(Self {
            current_frame_state: None,
            meshes: vec![],
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
            self.meshes
                .iter_mut()
                .for_each(|resources| self.device.destory_resource_pack(resources));
            self.device.destory_graphics_pipeline(&mut self.pipeline);
            self.device.destroy_swapchain(&mut self.swapchain);
            self.device.destory_render_pass(&mut self.render_pass);
            self.device
                .destory_descriptor_set_layout::<CameraMatrices>();
            self.device.destory();
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

impl Renderer for VulkanRenderer {
    fn begin_frame(&mut self, camera: &CameraMatrices) -> Result<(), Box<dyn Error>> {
        let (command, swapchain_frame) = self.device.begin_frame(&mut self.swapchain, camera)?;
        let command = self.device.record_command(command, |command| {
            command
                .begin_render_pass(&swapchain_frame, &self.render_pass)
                .bind_pipeline(&self.pipeline)
                // Camera descriptor set shoould be bound in VulkanSwapchain::begin_frame,
                // or VulkanSwapchain should not manage Camera uniform buffers
                .bind_camera_uniform_buffer(&self.pipeline, &swapchain_frame)
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
        let meshes = self.meshes.last().unwrap();
        Ok((0..meshes.meshes.len() as u32)
            .map(|mesh_index| {
                VulkanMeshHandle {
                    mesh_pack_index,
                    mesh_index,
                }
                .into()
            })
            .collect())
    }

    fn draw(&mut self, mesh: MeshHandle, transform: &Matrix4) -> Result<(), Box<dyn Error>> {
        let VulkanMeshHandle {
            mesh_pack_index,
            mesh_index,
        } = mesh.into();
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

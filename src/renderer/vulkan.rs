mod debug;
mod device;
mod surface;

use crate::math::types::Matrix4;

use self::device::{pipeline::GraphicsPipeline, resources::ResourcePack};

use super::{
    mesh::{Mesh, MeshHandle},
    Renderer,
};
use ash::{vk, Entry, Instance};
use bytemuck::bytes_of;
use debug::VulkanDebugUtils;
use device::{
    render_pass::VulkanRenderPass,
    swapchain::{SwapchainFrame, VulkanSwapchain},
    VulkanDevice,
};
use std::{
    error::Error,
    ffi::{c_char, CStr},
    mem::size_of,
    path::Path,
    result::Result,
};
use surface::VulkanSurface;
use winit::window::Window;

struct FrameState {
    swapchain_frame: SwapchainFrame,
    resource_pack_index: Option<u32>,
}

pub(super) struct VulkanRenderer {
    current_frame_state: Option<FrameState>,
    resources: Vec<ResourcePack>,
    pipeline: GraphicsPipeline,
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
            .find(|sup| unsafe { CStr::from_ptr(&sup.extension_name as *const _) } == req)
            .is_some()
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
            .find(|sup| unsafe { CStr::from_ptr(&sup.layer_name as *const _) } == req)
            .is_some()
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
            VulkanDebugUtils::required_layers()
                .into_iter()
                .map(|&req| req),
        )?;
        let enabled_extension_names = check_required_extension_support(
            &entry,
            VulkanDebugUtils::required_extensions()
                .into_iter()
                .chain(VulkanSurface::required_extensions())
                .map(|&req| req),
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
        let pipeline = device.create_graphics_pipeline(
            &render_pass,
            &swapchain,
            &[
                Path::new("shaders/spv/unlit/vert.spv"),
                Path::new("shaders/spv/unlit/frag.spv"),
            ],
        )?;
        Ok(Self {
            current_frame_state: None,
            resources: vec![],
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
            self.resources
                .iter_mut()
                .for_each(|resources| self.device.destory_resource_pack(resources));
            self.device.destory_graphics_pipeline(&mut self.pipeline);
            self.device.destroy_swapchain(&mut self.swapchain);
            self.device.destory_render_pass(&mut self.render_pass);
            self.device.destory();
            self.surface.destroy();
            drop(self.debug_utils.take());
            self.instance.destroy_instance(None);
        }
    }
}

struct VulkanMeshHandle {
    resource_pack_index: u32,
    mesh_index: u32,
}

impl From<MeshHandle> for VulkanMeshHandle {
    fn from(value: MeshHandle) -> Self {
        Self {
            resource_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            mesh_index: (0x00000000FFFFFFFF & value.0) as u32,
        }
    }
}

impl From<VulkanMeshHandle> for MeshHandle {
    fn from(value: VulkanMeshHandle) -> Self {
        Self(((value.resource_pack_index as u64) << 32) + value.mesh_index as u64)
    }
}

impl Renderer for VulkanRenderer {
    fn begin_frame(&mut self, view: &Matrix4, proj: &Matrix4) -> Result<(), Box<dyn Error>> {
        self.current_frame_state.replace(FrameState {
            swapchain_frame: self.device.begin_frame(&mut self.swapchain)?,
            resource_pack_index: None,
        });
        let frame_state = self.current_frame_state.as_ref().unwrap();
        let command_buffer = frame_state.swapchain_frame.command_buffer;
        self.device
            .begin_render_pass(&frame_state.swapchain_frame, &self.render_pass);
        self.device.bind_pipeline(command_buffer, &self.pipeline);
        self.device.push_constants(
            command_buffer,
            &self.pipeline,
            vk::ShaderStageFlags::VERTEX,
            0,
            bytes_of(proj),
        );
        self.device.push_constants(
            command_buffer,
            &self.pipeline,
            vk::ShaderStageFlags::VERTEX,
            size_of::<Matrix4>() * 1,
            bytes_of(view),
        );
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let frame_state = self
            .current_frame_state
            .take()
            .ok_or("current_frame is None!")?;
        self.device.end_render_pass(&frame_state.swapchain_frame);
        self.device
            .end_frame(&mut self.swapchain, frame_state.swapchain_frame)?;
        Ok(())
    }

    fn load_meshes(&mut self, meshes: &[Mesh]) -> Result<Vec<MeshHandle>, Box<dyn Error>> {
        let resource_pack_index = self.resources.len() as u32;
        self.resources.push(self.device.load_resource_pack(meshes)?);
        let resources = self.resources.last().unwrap();
        Ok((0..resources.meshes.len() as u32)
            .map(|mesh_index| {
                VulkanMeshHandle {
                    resource_pack_index,
                    mesh_index,
                }
                .into()
            })
            .collect())
    }

    fn draw(&mut self, mesh: MeshHandle, transform: &Matrix4) -> Result<(), Box<dyn Error>> {
        let VulkanMeshHandle {
            resource_pack_index,
            mesh_index,
        } = mesh.into();
        let frame_state = self
            .current_frame_state
            .as_mut()
            .take()
            .ok_or("current_frame is None!")?;
        if frame_state.resource_pack_index.is_none()
            || frame_state
                .resource_pack_index
                .is_some_and(|current_resources| current_resources != resource_pack_index)
        {
            self.device.use_resource_pack(
                frame_state.swapchain_frame.command_buffer,
                &self.resources[resource_pack_index as usize],
            );
            frame_state.resource_pack_index = Some(resource_pack_index);
        }
        let resources = &self.resources[resource_pack_index as usize];
        let command_buffer = frame_state.swapchain_frame.command_buffer;
        self.device.push_constants(
            command_buffer,
            &self.pipeline,
            vk::ShaderStageFlags::VERTEX,
            size_of::<Matrix4>() * 2,
            bytes_of(transform),
        );
        self.device
            .draw(command_buffer, resources, mesh_index as usize);
        Ok(())
    }
}

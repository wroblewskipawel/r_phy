mod debug;
mod device;
mod surface;

use self::device::{mesh::VulkanMesh, pipeline::GraphicsPipeline};

use super::{
    mesh::{Mesh, MeshHandle},
    Renderer,
};
use ash::{vk, Entry, Instance};
use debug::VulkanDebugUtils;
use device::{
    render_pass::VulkanRenderPass,
    swapchain::{Frame, VulkanSwapchain},
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
pub(super) struct VulkanRenderer {
    meshes: Vec<VulkanMesh>,
    current_frame: Option<Frame>,
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
            meshes: vec![],
            current_frame: None,
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
                .for_each(|mesh| self.device.destory_mesh(mesh));
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

impl Renderer for VulkanRenderer {
    fn begin_frame(&mut self) -> Result<(), Box<dyn Error>> {
        self.current_frame
            .replace(self.device.begin_frame(&mut self.swapchain)?);
        let frame = self.current_frame.as_ref().unwrap();
        self.device.begin_render_pass(frame, &self.render_pass);
        self.device.bind_pipeline(frame, &self.pipeline);
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let frame = self.current_frame.take().ok_or("current_frame is None!")?;
        self.device.end_render_pass(&frame);
        self.device.end_frame(&mut self.swapchain, frame)?;
        Ok(())
    }

    fn load_mesh(&mut self, mesh: &Mesh) -> Result<MeshHandle, Box<dyn Error>> {
        self.meshes.push(self.device.load_mesh(mesh)?);
        Ok(MeshHandle(self.meshes.len() - 1))
    }
    fn draw(&mut self, mesh: MeshHandle) -> Result<(), Box<dyn Error>> {
        let frame = self
            .current_frame
            .as_ref()
            .take()
            .ok_or("current_frame is None!")?;
        let mesh = &self.meshes[mesh.0];
        self.device.draw(frame, mesh);
        Ok(())
    }
}

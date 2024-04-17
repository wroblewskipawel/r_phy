mod debug;
mod device;
mod surface;

use crate::math::types::Matrix4;

use self::device::{
    frame::{FrameData, FramePool},
    framebuffer::presets::AttachmentsGBuffer,
    material::MaterialPack,
    mesh::MeshPack,
    render_pass::DeferedRenderPass,
    renderer::deferred::DeferredRenderer,
};

use super::{
    camera::Camera,
    model::{Material, MaterialHandle, Mesh, MeshHandle, Model},
    Renderer,
};
use ash::{vk, Entry, Instance};
use debug::VulkanDebugUtils;
use device::{swapchain::VulkanSwapchain, VulkanDevice};
use std::{
    error::Error,
    ffi::{c_char, CStr},
};
use surface::VulkanSurface;
use winit::window::Window;

pub(super) struct VulkanRenderer {
    current_frame: Option<FrameData<DeferredRenderer>>,
    materials: Vec<MaterialPack>,
    meshes: Vec<MeshPack>,
    frames: FramePool,
    renderer: DeferredRenderer,
    swapchain: VulkanSwapchain<AttachmentsGBuffer>,
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

// TODO: Error handling should be improved - currently when shader source files are missing,
// execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
// while error message indicating true cause of the issue is never presented to the user
// TODO: User should be able to load custom shareds,
// while also some preset of preconfigured one should be available
// API for user-defined shaders should be based on PipelineLayoutBuilder type-list
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
        let mut renderer = device.create_deferred_renderer()?;
        let swapchain = device.create_swapchain::<AttachmentsGBuffer>(
            &instance,
            &surface,
            |swapchain_image, extent| {
                device.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                    renderer.get_framebuffer_builder(swapchain_image),
                    extent,
                )
            },
        )?;
        device.update_deferred_renderer_input_descriptors(&mut renderer, &swapchain);
        let frames = device.create_frame_pool::<DeferredRenderer>(&swapchain)?;

        Ok(Self {
            current_frame: None,
            materials: vec![],
            meshes: vec![],
            frames,
            renderer,
            swapchain,
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
            self.device.destroy_deferred_renderer(&mut self.renderer);
            self.device.destory_frame_pool(&mut self.frames);
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

pub struct VulkanMeshHandle {
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

pub struct VulkanMaterialHandle {
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
        let frame = self.device.next_frame(
            &mut self.frames,
            &self.renderer,
            &self.swapchain,
            camera_matrices,
        )?;
        self.current_frame.replace(frame);
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let frame = self.current_frame.take().ok_or("current_frame is None!")?;
        self.device
            .end_frame(&self.renderer, frame, &self.swapchain)?;
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
        let frame = self.current_frame.take().ok_or("current_frame is None!")?;
        let frame = self.device.draw_mesh(
            &self.renderer,
            frame,
            transform,
            mesh.into(),
            material.into(),
            &self.meshes,
            &self.materials,
        );
        self.current_frame.replace(frame);
        Ok(())
    }
}

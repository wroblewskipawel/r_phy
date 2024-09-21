use std::error::Error;
use std::ffi::{c_char, CStr};
use std::ops::{Deref, DerefMut};

use ash::{vk, Entry, Instance};
use winit::window::Window;

use super::debug::VulkanDebugUtils as DebugUtils;
use super::device::VulkanDevice as Device;
use super::surface::VulkanSurface as Surface;

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

pub(super) struct Context {
    device: Device,
    pub surface: Surface,
    debug_utils: Option<DebugUtils>,
    pub instance: Instance,
    _entry: Entry,
}

impl Context {
    pub fn build(window: &Window) -> Result<Self, Box<dyn Error>> {
        let entry = unsafe { Entry::load()? };
        let enabled_layer_names =
            check_required_layer_support(&entry, DebugUtils::required_layers().iter().copied())?;
        let enabled_extension_names = check_required_extension_support(
            &entry,
            DebugUtils::required_extensions()
                .iter()
                .chain(Surface::required_extensions())
                .copied(),
        )?;
        let application_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_1,
            ..Default::default()
        };
        let mut debug_messenger_info = DebugUtils::create_info();
        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&enabled_layer_names)
            .enabled_extension_names(&enabled_extension_names)
            .push_next(&mut debug_messenger_info);
        let instance = unsafe { entry.create_instance(&create_info, None)? };
        let debug_utils = DebugUtils::build(&entry, &instance)?;
        let surface = Surface::create(&entry, &instance, window)?;
        let device = Device::create(&instance, &surface)?;

        Ok(Self {
            device,
            surface,
            debug_utils: Some(debug_utils),
            instance,
            _entry: entry,
        })
    }
}

impl<'a> From<&'a Context> for &'a Device {
    fn from(context: &'a Context) -> Self {
        &context.device
    }
}

impl<'a> From<&'a Context> for &'a Surface {
    fn from(context: &'a Context) -> Self {
        &context.surface
    }
}

impl<'a> From<&'a Context> for &'a Instance {
    fn from(context: &'a Context) -> Self {
        &context.instance
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _ = self.device.wait_idle();
        unsafe {
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

impl Deref for Context {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}

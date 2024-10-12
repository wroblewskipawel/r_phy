mod debug;
pub mod device;
mod surface;

use std::error::Error;
use std::ffi::{c_char, CStr};
use std::ops::{Deref, DerefMut};

use ash::{vk, Entry, Instance};
use winit::window::Window;

#[cfg(debug_assertions)]
use debug::DebugUtils;
use device::Device;
use surface::VulkanSurface as Surface;

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

pub struct Context {
    device: Device,
    surface: Surface,
    #[cfg(debug_assertions)]
    debug_utils: DebugUtils,
    instance: Instance,
    _entry: Entry,
}

impl Context {
    pub fn build(window: &Window) -> Result<Self, Box<dyn Error>> {
        let entry = unsafe { Entry::load()? };
        let required_extensions = Surface::iterate_required_extensions();

        #[cfg(debug_assertions)]
        let required_extensions =
            required_extensions.chain(DebugUtils::iterate_required_extensions());

        let enabled_extension_names =
            check_required_extension_support(&entry, required_extensions)?;

        #[cfg(debug_assertions)]
        let enabled_layer_names = DebugUtils::check_required_layer_support(&entry)?;

        let application_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_1,
            ..Default::default()
        };

        #[cfg(debug_assertions)]
        let mut debug_messenger_info = DebugUtils::create_info();

        let create_info = {
            #[cfg(debug_assertions)]
            {
                vk::InstanceCreateInfo::builder()
                    .push_next(&mut debug_messenger_info)
                    .enabled_layer_names(&enabled_layer_names)
            }
            #[cfg(not(debug_assertions))]
            {
                vk::InstanceCreateInfo::builder()
            }
        };

        let create_info = create_info
            .application_info(&application_info)
            .enabled_extension_names(&enabled_extension_names);
        let instance = unsafe { entry.create_instance(&create_info, None)? };

        #[cfg(debug_assertions)]
        let debug_utils = DebugUtils::build(&entry, &instance)?;

        let surface = Surface::create(&entry, &instance, window)?;
        let device = Device::create(&instance, &surface)?;

        Ok(Self {
            device,
            surface,
            #[cfg(debug_assertions)]
            debug_utils,
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
            #[cfg(debug_assertions)]
            self.debug_utils.destroy();
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

mod debug;
pub mod device;
mod surface;

use ash::extensions::{ext, khr};
use std::error::Error;
use std::ffi::{c_char, CStr};
use std::ops::{Deref, DerefMut};
use type_kit::{Destroy, DropGuard, Finalize};

use ash::vk;
use winit::window::Window;

#[cfg(debug_assertions)]
use debug::DebugUtils;
use device::Device;
use surface::Surface;

fn check_required_extension_support(
    entry: &ash::Entry,
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

struct Instance {
    instance: ash::Instance,
    _entry: ash::Entry,
}

trait InstanceExtension: Sized {
    fn load(entry: &ash::Entry, instance: &ash::Instance) -> Self;
}

impl InstanceExtension for ext::DebugUtils {
    #[inline]
    fn load(entry: &ash::Entry, instance: &ash::Instance) -> Self {
        Self::new(entry, instance)
    }
}

impl InstanceExtension for khr::Surface {
    #[inline]
    fn load(entry: &ash::Entry, instance: &ash::Instance) -> Self {
        Self::new(entry, instance)
    }
}

impl InstanceExtension for khr::Win32Surface {
    #[inline]
    fn load(entry: &ash::Entry, instance: &ash::Instance) -> Self {
        Self::new(entry, instance)
    }
}

impl Instance {
    fn create() -> Result<Self, Box<dyn Error>> {
        let entry = unsafe { ash::Entry::load()? };
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
        Ok(Self {
            instance,
            _entry: entry,
        })
    }

    #[inline]
    pub(crate) fn load<E: InstanceExtension>(&self) -> E {
        E::load(&self._entry, &self.instance)
    }
}

impl Deref for Instance {
    type Target = ash::Instance;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl DerefMut for Instance {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl Destroy for Instance {
    type Context<'a> = ();

    #[inline]
    fn destroy<'a>(&mut self, _context: Self::Context<'a>) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

pub struct Context {
    device: DropGuard<Device>,
    surface: DropGuard<Surface>,
    #[cfg(debug_assertions)]
    debug_utils: DropGuard<DebugUtils>,
    instance: DropGuard<Instance>,
}

trait DeviceExtension: Sized {
    fn load(instance: &ash::Instance, device: &ash::Device) -> Self;
}

impl DeviceExtension for khr::Swapchain {
    #[inline]
    fn load(instance: &ash::Instance, device: &ash::Device) -> Self {
        Self::new(instance, device)
    }
}

impl Context {
    pub fn build(window: &Window) -> Result<Self, Box<dyn Error>> {
        let instance = Instance::create()?;
        #[cfg(debug_assertions)]
        let debug_utils = DebugUtils::create(&instance)?;
        let surface = Surface::create(&instance, window)?;
        let device = Device::create(&instance, &surface)?;

        Ok(Self {
            device: DropGuard::new(device),
            surface: DropGuard::new(surface),
            #[cfg(debug_assertions)]
            debug_utils: DropGuard::new(debug_utils),
            instance: DropGuard::new(instance),
        })
    }

    #[inline]
    pub(crate) fn load<E: DeviceExtension>(&self) -> E {
        E::load(&self.instance, &self.device)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _ = self.device.wait_idle();
        self.device.finalize();
        self.surface.finalize();
        #[cfg(debug_assertions)]
        self.debug_utils.finalize();
        self.instance.finalize();
    }
}

impl Deref for Context {
    type Target = Device;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl DerefMut for Context {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}

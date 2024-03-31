#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;
use ash::{extensions::khr::Surface, vk, Entry, Instance};
use std::{
    collections::HashSet,
    error::Error,
    ffi::{c_void, CStr},
    ptr::null,
};
#[cfg(target_os = "windows")]
use winit::raw_window_handle::Win32WindowHandle;
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

pub(super) struct VulkanSurface {
    handle: vk::SurfaceKHR,
    loader: Surface,
}

#[cfg(target_os = "windows")]
fn create_platform_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, Box<dyn Error>> {
    let win32_surface = Win32Surface::new(entry, instance);
    let (hwnd, hinstance) = match window.window_handle()?.as_raw() {
        RawWindowHandle::Win32(Win32WindowHandle {
            hwnd, hinstance, ..
        }) => {
            let hwnd = hwnd.get() as *const c_void;
            let hinstance = hinstance.map_or(null(), |hinstance| hinstance.get() as *const c_void);
            (hwnd, hinstance)
        }
        _ => panic!("Unexpected RawWindowHandleType for current platform!"),
    };
    let handle = unsafe {
        win32_surface.create_win32_surface(
            &vk::Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd)
                .hinstance(hinstance),
            None,
        )?
    };
    Ok(handle)
}

#[cfg(not(target_os = "windows"))]
fn create_platform_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, Box<dyn Error>> {
    compile_error!("Current platform not supported!");
}

impl VulkanSurface {
    #[cfg(target_os = "windows")]
    pub fn required_extensions() -> &'static [&'static CStr; 2] {
        const REQUIRED_EXTENSIONS: &[&CStr; 2] = &[Win32Surface::name(), Surface::name()];
        REQUIRED_EXTENSIONS
    }

    #[cfg(not(target_os = "windows"))]
    pub fn required_extensions() -> &'static [&'static CStr; 2] {
        compile_error!("Current platform not supported!");
    }

    pub fn create(
        entry: &Entry,
        instance: &Instance,
        window: &Window,
    ) -> Result<Self, Box<dyn Error>> {
        let handle = create_platform_surface(entry, instance, window)?;
        let loader = Surface::new(entry, instance);
        Ok(Self { handle, loader })
    }

    pub fn destroy(&mut self) {
        unsafe { self.loader.destroy_surface(self.handle, None) };
    }
}

impl From<&VulkanSurface> for vk::SurfaceKHR {
    fn from(value: &VulkanSurface) -> Self {
        value.handle
    }
}

pub(super) struct PhysicalDeviceSurfaceProperties {
    pub present_mode: vk::PresentModeKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub supported_queue_families: HashSet<u32>,
    pub capabilities: vk::SurfaceCapabilitiesKHR,
}

impl PhysicalDeviceSurfaceProperties {
    const PREFERRED_SURFACE_FORMATS: &'static [vk::Format] =
        &[vk::Format::R8G8B8A8_SRGB, vk::Format::B8G8R8A8_SRGB];

    pub fn get(
        surface: &VulkanSurface,
        physical_device: vk::PhysicalDevice,
        quque_families: &[(vk::QueueFamilyProperties, u32)],
    ) -> Result<Self, Box<dyn Error>> {
        let surface_formats = unsafe {
            surface
                .loader
                .get_physical_device_surface_formats(physical_device, surface.handle)?
        };
        let surface_format = *Self::PREFERRED_SURFACE_FORMATS
            .iter()
            .find_map(|&pref| {
                surface_formats.iter().find(|supported| {
                    supported.format == pref
                        && supported.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
            })
            .or(surface_formats.first())
            .ok_or("Failed to pick surface format for physical device!")?;
        let present_mode = unsafe {
            surface
                .loader
                .get_physical_device_surface_present_modes(physical_device, surface.handle)?
        }
        .into_iter()
        .find(|&present_mode| present_mode == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);
        let supported_queue_families = HashSet::<u32>::from_iter(
            quque_families
                .iter()
                .filter(|&&(properties, queue_family_index)| {
                    properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && unsafe {
                            surface
                                .loader
                                .get_physical_device_surface_support(
                                    physical_device,
                                    queue_family_index,
                                    surface.handle,
                                )
                                .unwrap_or(false)
                        }
                })
                .map(|&(_, queue_family_index)| queue_family_index),
        );
        let capabilities = unsafe {
            surface
                .loader
                .get_physical_device_surface_capabilities(physical_device, surface.handle)?
        };
        Ok(Self {
            present_mode,
            surface_format,
            supported_queue_families,
            capabilities,
        })
    }

    pub fn get_current_extent(&self) -> vk::Extent2D {
        let vk::SurfaceCapabilitiesKHR {
            current_extent,
            min_image_extent,
            max_image_extent,
            ..
        } = self.capabilities;
        vk::Extent2D {
            width: current_extent
                .width
                .clamp(min_image_extent.width, max_image_extent.width),
            height: current_extent
                .height
                .clamp(min_image_extent.height, max_image_extent.height),
        }
    }

    pub fn get_image_count(&self) -> u32 {
        let vk::SurfaceCapabilitiesKHR {
            min_image_count,
            max_image_count,
            ..
        } = self.capabilities;
        (min_image_count + 1).clamp(
            0,
            match max_image_count {
                0 => u32::MAX,
                _ => max_image_count,
            },
        )
    }
}

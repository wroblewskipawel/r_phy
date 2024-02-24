#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;
use ash::{extensions::khr::Surface, vk, Entry, Instance};
use std::{
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

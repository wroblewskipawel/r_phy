use std::{
    error::Error,
    ffi::{c_void, CStr},
};

use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};
use colored::{self, Colorize};

unsafe extern "system" fn debug_messenger_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    message: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    let message_severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "ERROR".red(),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "WARNING".yellow(),
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "INFO".blue(),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "VERBOSE".dimmed(),
        _ => "UNKNOWN".magenta(),
    }
    .bold();
    let message_type = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "GENERAL".blue(),
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "PERFORMANCE".yellow(),
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "VALIDATION".red(),
        vk::DebugUtilsMessageTypeFlagsEXT::DEVICE_ADDRESS_BINDING => {
            "DEVICE_ADDRESS_BINDING".dimmed()
        }
        _ => "UNKNOWN".magenta(),
    }
    .bold();
    let message = CStr::from_ptr((*message).p_message).to_string_lossy();
    println!("[{}][{}]:{}", message_severity, message_type, message);
    vk::FALSE
}

pub(super) struct VulkanDebugUtils {
    messenger: vk::DebugUtilsMessengerEXT,
    loader: DebugUtils,
}

impl VulkanDebugUtils {
    pub fn create_info() -> vk::DebugUtilsMessengerCreateInfoEXT {
        vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
            pfn_user_callback: Some(debug_messenger_callback),
            ..Default::default()
        }
    }

    pub fn required_layers() -> &'static [&'static CStr; 1] {
        const REQUIRED_LAYERS: &[&CStr; 1] =
            &[unsafe { &CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }];
        REQUIRED_LAYERS
    }

    pub fn required_extensions() -> &'static [&'static CStr; 1] {
        const REQUIRED_EXTENSIONS: &[&CStr; 1] = &[DebugUtils::name()];
        REQUIRED_EXTENSIONS
    }

    pub fn build(entry: &Entry, instance: &Instance) -> Result<VulkanDebugUtils, Box<dyn Error>> {
        let loader = DebugUtils::new(entry, instance);
        let messenger = unsafe { loader.create_debug_utils_messenger(&Self::create_info(), None)? };
        Ok(Self { messenger, loader })
    }
}

impl Drop for VulkanDebugUtils {
    fn drop(&mut self) {
        unsafe {
            self.loader
                .destroy_debug_utils_messenger(self.messenger, None);
        }
    }
}

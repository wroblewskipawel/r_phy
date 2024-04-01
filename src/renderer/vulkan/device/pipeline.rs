mod graphics;
mod layout;

pub use graphics::*;
pub use layout::*;

use ash::vk;
use std::{error::Error, ffi::CStr, path::Path};

struct ShaderModule {
    module: vk::ShaderModule,
    stage: vk::ShaderStageFlags,
}

impl ShaderModule {
    const ENTRY_POINT: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"main\0") };

    fn get_stage_create_info(&self) -> vk::PipelineShaderStageCreateInfo {
        vk::PipelineShaderStageCreateInfo {
            module: self.module,
            stage: self.stage,
            p_name: Self::ENTRY_POINT.as_ptr(),
            ..Default::default()
        }
    }

    fn get_shader_stage(path: &Path) -> Result<vk::ShaderStageFlags, Box<dyn Error>> {
        match path.file_stem().map(|stem| stem.to_str().unwrap_or("")) {
            Some(stem) => match stem {
                "frag" => Ok(vk::ShaderStageFlags::FRAGMENT),
                "vert" => Ok(vk::ShaderStageFlags::VERTEX),
                stem => Err(format!(
                    "Invalid shader module path - unknown shader file type: {}!",
                    stem
                ))?,
            },
            None => Err("Invalid shader module path - mising file name component!")?,
        }
    }
}

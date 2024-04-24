mod graphics;
mod layout;
mod states;

pub use graphics::*;
pub use layout::*;
pub use states::*;

use ash::{vk, Device};
use std::{error::Error, ffi::CStr, marker::PhantomData, path::Path};

use super::VulkanDevice;

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

pub struct Modules<'a> {
    modules: Vec<ShaderModule>,
    device: &'a Device,
}

impl<'a> Drop for Modules<'a> {
    fn drop(&mut self) {
        unsafe {
            self.modules
                .iter()
                .for_each(|module| self.device.destroy_shader_module(module.module, None));
        }
    }
}

pub struct PipelineStagesInfo<'a> {
    stages: Vec<vk::PipelineShaderStageCreateInfo>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Modules<'a> {
    pub fn get_stages_info(&self) -> PipelineStagesInfo {
        PipelineStagesInfo {
            stages: self
                .modules
                .iter()
                .map(|module| module.get_stage_create_info())
                .collect(),
            _phantom: PhantomData,
        }
    }
}

pub trait ModuleLoader {
    fn load(self, device: &VulkanDevice) -> Result<Modules, Box<dyn Error>>;
}

pub struct ShaderDirectory<'a> {
    path: &'a Path,
}

impl<'a> ShaderDirectory<'a> {
    pub fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl<'a> ModuleLoader for ShaderDirectory<'a> {
    fn load(self, device: &VulkanDevice) -> Result<Modules, Box<dyn Error>> {
        let modules = Modules {
            modules: self
                .path
                .read_dir()?
                .flatten()
                .filter_map(|entry| {
                    entry
                        .file_type()
                        .is_ok_and(|f| f.is_file())
                        .then_some(device.load_shader_module(&entry.path()))
                })
                .collect::<Result<Vec<_>, _>>()?,
            device,
        };
        Ok(modules)
    }
}

impl VulkanDevice {
    fn load_shader_module(&self, path: &Path) -> Result<ShaderModule, Box<dyn Error>> {
        let code = std::fs::read(path)?;
        let stage = ShaderModule::get_shader_stage(path)?;
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as *const _,
            ..Default::default()
        };
        let module = unsafe { self.device.create_shader_module(&create_info, None)? };
        Ok(ShaderModule { module, stage })
    }
}

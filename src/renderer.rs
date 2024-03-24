pub mod camera;
pub mod mesh;
mod vulkan;

use mesh::{Mesh, MeshHandle};
use std::error::Error;
use vulkan::VulkanRenderer;
use winit::window::Window;

use crate::math::types::Matrix4;

use self::camera::Camera;

pub trait Renderer {
    fn begin_frame(&mut self, camera: &Camera) -> Result<(), Box<dyn Error>>;
    fn end_frame(&mut self) -> Result<(), Box<dyn Error>>;
    fn load_meshes(&mut self, mesh: &[Mesh]) -> Result<Vec<MeshHandle>, Box<dyn Error>>;
    fn draw(&mut self, mesh: MeshHandle, transform: &Matrix4) -> Result<(), Box<dyn Error>>;
}

pub enum RendererBackend {
    Vulkan,
}

impl RendererBackend {
    pub fn create(self, window: &Window) -> Result<Box<dyn Renderer>, Box<dyn Error>> {
        let renderer = match self {
            RendererBackend::Vulkan => Box::new(VulkanRenderer::new(window)?),
        };
        Ok(renderer)
    }
}

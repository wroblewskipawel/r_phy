pub mod camera;
pub mod model;
pub mod shader;
pub mod vulkan;

use model::{Material, MaterialHandle, MeshHandle, Vertex};
use shader::{ShaderHandle, ShaderType};
use std::error::Error;
use winit::window::Window;

use crate::math::types::Matrix4;

use self::{
    camera::Camera,
    model::{Drawable, MaterialTypeList, MaterialTypeTerminator, MeshList, MeshTerminator},
};

pub trait Renderer: 'static {
    type Materials: MaterialTypeList;
    type Meshes: MeshList;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>>;
    fn end_frame(&mut self) -> Result<(), Box<dyn Error>>;
    fn draw<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>>(
        &mut self,
        _shdaer: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>>;
    fn get_mesh_handles<V: Vertex>(&self) -> Option<Vec<MeshHandle<V>>>;
    fn get_material_handles<M: Material>(&self) -> Option<Vec<MaterialHandle<M>>>;
    fn get_shader_handles<S: ShaderType>(&self) -> Option<Vec<ShaderHandle<S>>>;
}

pub trait RendererBuilder: 'static {
    type Renderer: Renderer;
    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>>;
}

pub struct RendererNone;

impl Renderer for RendererNone {
    type Materials = MaterialTypeTerminator;
    type Meshes = MeshTerminator;

    fn begin_frame<C: Camera>(&mut self, _camera: &C) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn draw<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>>(
        &mut self,
        _shdaer: ShaderHandle<S>,
        _drawable: &D,
        _transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn get_mesh_handles<V: Vertex>(&self) -> Option<Vec<MeshHandle<V>>> {
        unimplemented!()
    }

    fn get_material_handles<M: Material>(&self) -> Option<Vec<MaterialHandle<M>>> {
        unimplemented!()
    }

    fn get_shader_handles<S: ShaderType>(&self) -> Option<Vec<ShaderHandle<S>>> {
        unimplemented!()
    }
}

impl RendererBuilder for RendererNone {
    type Renderer = Self;

    fn build(self, _window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        panic!("Renderer Type not provided!")
    }
}

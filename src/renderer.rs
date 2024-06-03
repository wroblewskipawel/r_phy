pub mod camera;
pub mod model;
pub mod vulkan;

use model::{Material, MaterialHandle, MeshHandle, Vertex};
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
    fn draw<D: Drawable>(
        &mut self,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>>;
    fn get_mesh_handles<V: Vertex>(&self) -> Option<Vec<MeshHandle<V>>>;
    fn get_material_handles<M: Material>(&self) -> Option<Vec<MaterialHandle<M>>>;
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

    fn draw<D: Drawable>(
        &mut self,
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
}

impl RendererBuilder for RendererNone {
    type Renderer = Self;

    fn build(self, _window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        panic!("Renderer Type not provided!")
    }
}

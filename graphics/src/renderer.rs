pub mod camera;
pub mod vulkan;

use math::types::Matrix4;
use std::error::Error;
use type_kit::Nil;
use winit::window::Window;

use to_resolve::{
    model::Drawable,
    shader::{ShaderHandle, ShaderType},
};

use self::camera::Camera;

pub trait Renderer: 'static {}

pub trait ContextBuilder {
    type Renderer: Renderer;
    type Context: RendererContext<Renderer = Self::Renderer>;

    fn build(self, renderer: &Self::Renderer) -> Result<Self::Context, Box<dyn Error>>;
}

pub trait RendererContext: 'static {
    type Renderer: Renderer;
    type Shaders;
    type Materials;
    type Meshes;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>>;
    fn end_frame(&mut self) -> Result<(), Box<dyn Error>>;
    fn draw<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>>(
        &mut self,
        shader: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>>;
}

pub trait RendererBuilder: 'static {
    type Renderer: Renderer;
    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>>;
}

impl Renderer for Nil {}

impl ContextBuilder for Nil {
    type Renderer = Nil;
    type Context = Nil;

    fn build(self, _renderer: &Self::Renderer) -> Result<Self::Context, Box<dyn Error>> {
        Ok(Nil::new())
    }
}

impl RendererContext for Nil {
    type Renderer = Nil;
    type Shaders = Nil;
    type Materials = Nil;
    type Meshes = Nil;

    fn begin_frame<C: Camera>(&mut self, _camera: &C) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn draw<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>>(
        &mut self,
        _shader: ShaderHandle<S>,
        _drawable: &D,
        _transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }
}

impl RendererBuilder for Nil {
    type Renderer = Self;

    fn build(self, _window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        panic!("Renderer Type not provided!")
    }
}

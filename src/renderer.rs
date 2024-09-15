pub mod camera;
pub mod model;
pub mod shader;
pub mod vulkan;

use model::{Material, MaterialHandle, Mesh, MeshHandle, Vertex};
use shader::{ShaderHandle, ShaderType, ShaderTypeList};
use std::error::Error;
use winit::window::Window;

use crate::{
    core::{Contains, Marker, Nil},
    math::types::Matrix4,
};

use self::{
    camera::Camera,
    model::{Drawable, MaterialTypeList, MeshTypeList},
};

pub trait Renderer: 'static {
    type Builder: ContextBuilder;
    type Context: RendererContext;

    fn load_context(&mut self, builder: Self::Builder) -> Result<Self::Context, Box<dyn Error>>;
}

pub trait RendererContext: 'static {
    type Shaders: ShaderTypeList;
    type Materials: MaterialTypeList;
    type Meshes: MeshTypeList;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>>;
    fn end_frame(&mut self) -> Result<(), Box<dyn Error>>;
    fn draw<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>>(
        &mut self,
        shader: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>>;
}

pub trait ContextBuilder: Default {
    type Shaders: ShaderTypeList;
    type Materials: MaterialTypeList;
    type Meshes: MeshTypeList;

    fn add_material<N: Material, T: Marker>(&mut self, material: N) -> MaterialHandle<N>
    where
        Self::Materials: Contains<Vec<N>, T>;

    fn add_mesh<N: Vertex, T: Marker>(&mut self, mesh: Mesh<N>) -> MeshHandle<N>
    where
        Self::Meshes: Contains<Vec<Mesh<N>>, T>;
    fn add_shader<N: ShaderType, O: ShaderType, T: Marker>(&mut self, shader: N) -> ShaderHandle<N>
    where
        O: From<N>,
        Self::Shaders: Contains<Vec<O>, T>;
}

pub trait RendererBuilder: 'static {
    type Renderer: Renderer;
    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>>;
}

#[derive(Debug, Default)]
pub struct RendererContextBuilder<S: ShaderTypeList, M: MaterialTypeList, V: MeshTypeList> {
    shaders: S,
    materials: M,
    meshes: V,
}

fn push_and_get_index<V>(vec: &mut Vec<V>, value: V) -> u32 {
    let index = vec.len();
    vec.push(value);
    index.try_into().unwrap()
}

impl<S: ShaderTypeList + Default, M: MaterialTypeList + Default, V: MeshTypeList + Default>
    ContextBuilder for RendererContextBuilder<S, M, V>
{
    type Shaders = S;
    type Materials = M;
    type Meshes = V;

    fn add_material<N: Material, T: Marker>(&mut self, material: N) -> MaterialHandle<N>
    where
        Self::Materials: Contains<Vec<N>, T>,
    {
        MaterialHandle::new(push_and_get_index(self.materials.get_mut(), material))
    }

    fn add_mesh<N: Vertex, T: Marker>(&mut self, mesh: Mesh<N>) -> MeshHandle<N>
    where
        Self::Meshes: Contains<Vec<Mesh<N>>, T>,
    {
        MeshHandle::new(push_and_get_index(self.meshes.get_mut(), mesh))
    }

    fn add_shader<N: ShaderType, O: ShaderType, T: Marker>(&mut self, shader: N) -> ShaderHandle<N>
    where
        O: From<N>,
        Self::Shaders: Contains<Vec<O>, T>,
    {
        ShaderHandle::new(push_and_get_index(self.shaders.get_mut(), shader.into()))
    }
}

impl Renderer for Nil {
    type Builder = RendererContextBuilder<Nil, Nil, Nil>;
    type Context = Nil;

    fn load_context(&mut self, _builder: Self::Builder) -> Result<Self::Context, Box<dyn Error>> {
        unimplemented!()
    }
}

impl RendererContext for Nil {
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

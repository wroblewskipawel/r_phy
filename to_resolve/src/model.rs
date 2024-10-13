mod gltf;
mod material;
mod mesh;

use std::fmt::Debug;

pub use material::*;
pub use mesh::*;
use type_kit::Nil;

pub trait DrawableType: 'static {
    type Vertex: Vertex;
    type Material: Material;
}

pub trait Drawable: DrawableType {
    fn material(&self) -> MaterialHandle<Self::Material>;
    fn mesh(&self) -> MeshHandle<Self::Vertex>;
}

#[derive(Debug)]
pub struct Model<M: Material, V: Vertex> {
    pub mesh: MeshHandle<V>,
    pub material: MaterialHandle<M>,
}

impl<M: Material, V: Vertex> Clone for Model<M, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Material, V: Vertex> Copy for Model<M, V> {}

impl<M: Material, V: Vertex> Model<M, V> {
    pub fn new(mesh: MeshHandle<V>, material: MaterialHandle<M>) -> Self {
        Self { mesh, material }
    }
}

impl<M: Material, V: Vertex> DrawableType for Model<M, V> {
    type Vertex = V;
    type Material = M;
}

impl<M: Material, V: Vertex> Drawable for Model<M, V> {
    fn material(&self) -> MaterialHandle<Self::Material> {
        self.material
    }

    fn mesh(&self) -> MeshHandle<Self::Vertex> {
        self.mesh
    }
}

impl DrawableType for Nil {
    type Vertex = VertexNone;
    type Material = EmptyMaterial;
}

impl Drawable for Nil {
    fn material(&self) -> MaterialHandle<Self::Material> {
        unreachable!()
    }

    fn mesh(&self) -> MeshHandle<Self::Vertex> {
        unreachable!()
    }
}

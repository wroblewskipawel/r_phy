mod gltf;
mod material;
mod mesh;

use std::fmt::Debug;

pub use material::*;
pub use mesh::*;

pub trait Drawable: 'static {
    type Vertex: Vertex;
    type Material: Material;

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

impl<M: Material, V: Vertex> Drawable for Model<M, V> {
    type Vertex = V;
    type Material = M;

    fn material(&self) -> MaterialHandle<Self::Material> {
        self.material
    }

    fn mesh(&self) -> MeshHandle<Self::Vertex> {
        self.mesh
    }
}

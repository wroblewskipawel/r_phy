mod material;
mod mesh;

pub use material::*;
pub use mesh::*;

#[derive(Debug, Clone, Copy)]
pub struct Model {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

impl Model {
    pub fn new(mesh: MeshHandle, material: MaterialHandle) -> Self {
        Self { mesh, material }
    }
}

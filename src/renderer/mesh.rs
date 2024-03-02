use bytemuck::{Pod, Zeroable};

use crate::math::types::Vector3;

#[derive(Debug, Clone, Copy)]
pub struct MeshHandle(pub usize);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
pub struct Vertex {
    pub(crate) pos: Vector3,
    pub(crate) color: Vector3,
}

pub struct Mesh {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
}

impl Mesh {
    pub fn triangle() -> Self {
        Mesh {
            vertices: vec![
                Vertex {
                    pos: Vector3::new(0.0, 0.5, 0.5),
                    color: Vector3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, 0.5),
                    color: Vector3::new(0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, 0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
            ],
            indices: vec![0, 2, 1],
        }
    }
}

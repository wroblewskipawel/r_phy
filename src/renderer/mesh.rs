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
                    pos: Vector3::new(0.0, 0.5, 0.0),
                    color: Vector3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, 0.0),
                    color: Vector3::new(0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, 0.0),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
            ],
            indices: vec![0, 2, 1],
        }
    }

    pub fn cube() -> Self {
        Mesh {
            vertices: vec![
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, -0.5),
                    color: Vector3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, -0.5),
                    color: Vector3::new(0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, 0.5, -0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, 0.5, -0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, 0.5),
                    color: Vector3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, 0.5),
                    color: Vector3::new(0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, 0.5, 0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, 0.5, 0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                },
            ],
            indices: vec![
                0, 3, 1, 1, 3, 2, 0, 1, 4, 4, 1, 5, 1, 2, 5, 5, 2, 6, 7, 4, 5, 7, 5, 6, 0, 4, 3, 4,
                7, 3, 7, 2, 3, 7, 6, 2,
            ],
        }
    }
}

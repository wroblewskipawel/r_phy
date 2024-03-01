#[derive(Debug, Clone, Copy)]
pub struct MeshHandle(pub usize);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub(crate) pos: (f32, f32, f32),
    pub(crate) color: (f32, f32, f32),
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
                    pos: (0.0, 0.5, 0.5),
                    color: (1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: (0.5, -0.5, 0.5),
                    color: (0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: (-0.5, -0.5, 0.5),
                    color: (0.0, 0.0, 1.0),
                },
            ],
            indices: vec![0, 2, 1],
        }
    }
}

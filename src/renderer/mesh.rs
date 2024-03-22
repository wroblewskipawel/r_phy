use bytemuck::{Pod, Zeroable};

use crate::{
    math::types::{Vector2, Vector3},
    physics::shape,
};

#[derive(Debug, Clone, Copy)]
pub struct MeshHandle(pub u64);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
pub struct Vertex {
    pub(crate) pos: Vector3,
    pub(crate) color: Vector3,
    pub(crate) norm: Vector3,
    pub(crate) uv: Vector2,
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
                    norm: Vector3::new(0.0, 0.0, 1.0),
                    uv: Vector2::new(0.5, 1.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, 0.0),
                    color: Vector3::new(0.0, 1.0, 0.0),
                    norm: Vector3::new(0.0, 0.0, 1.0),
                    uv: Vector2::new(1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, 0.0),
                    color: Vector3::new(0.0, 0.0, 1.0),
                    norm: Vector3::new(0.0, 0.0, 1.0),
                    uv: Vector2::new(0.0, 0.0),
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
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, -0.5),
                    color: Vector3::new(0.0, 1.0, 0.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, 0.5, -0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(1.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, 0.5, -0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(0.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, -0.5, 0.5),
                    color: Vector3::new(1.0, 0.0, 0.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(0.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, -0.5, 0.5),
                    color: Vector3::new(0.0, 1.0, 0.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(1.0, 0.0),
                },
                Vertex {
                    pos: Vector3::new(0.5, 0.5, 0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(1.0, 1.0),
                },
                Vertex {
                    pos: Vector3::new(-0.5, 0.5, 0.5),
                    color: Vector3::new(0.0, 0.0, 1.0),
                    norm: Vector3::new(0.0, 0.0, 0.0),
                    uv: Vector2::new(0.0, 1.0),
                },
            ],
            indices: vec![
                0, 3, 1, 1, 3, 2, 0, 1, 4, 4, 1, 5, 1, 2, 5, 5, 2, 6, 7, 4, 5, 7, 5, 6, 0, 4, 3, 4,
                7, 3, 7, 2, 3, 7, 6, 2,
            ],
        }
    }

    fn unit_cube_subdivided(num_subdiv: usize) -> Mesh {
        const FACES_BASES: &'static [(Vector3, Vector3, Vector3, Vector3)] = &[
            (
                Vector3::new(-0.5, -0.5, -0.5),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            (
                Vector3::new(-0.5, -0.5, -0.5),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ),
            (
                Vector3::new(-0.5, -0.5, -0.5),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
            (
                Vector3::new(0.5, 0.5, 0.5),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            (
                Vector3::new(0.5, 0.5, 0.5),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, -1.0),
                Vector3::new(0.0, 1.0, 0.0),
            ),
            (
                Vector3::new(0.5, 0.5, 0.5),
                Vector3::new(0.0, 0.0, -1.0),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
        ];

        let build_face = |origin: Vector3,
                          u: Vector3,
                          v: Vector3,
                          color: Vector3,
                          index_offset: usize|
         -> (Vec<Vertex>, Vec<u32>) {
            let num_side_vertices = 2 + num_subdiv;
            let num_face_vertices = num_side_vertices.pow(2);
            // Can Vec::with_capacity be used here?
            let mut vertices = vec![Vertex::default(); num_face_vertices];
            let num_side_quads = 1 + num_subdiv;
            let num_face_indices = num_side_quads.pow(2) * 6;
            let face_normal = u.cross(v).norm();
            // Try use iterator syntax and copare resulting assembly code
            for i in 0..num_side_vertices {
                for j in 0..num_side_vertices {
                    let u_scale = i as f32 / (num_side_vertices - 1) as f32;
                    let v_scale = j as f32 / (num_side_vertices - 1) as f32;
                    let vertex = &mut vertices[i * num_side_vertices + j];
                    vertex.pos = origin + u_scale * u + v_scale * v;
                    vertex.color = color;
                    vertex.norm = face_normal;
                    vertex.uv = Vector2::new(u_scale, v_scale);
                }
            }
            let mut indices = vec![0u32; num_face_indices];
            for i in 0..num_side_quads {
                for j in 0..num_side_quads {
                    let quad_index = i * num_side_quads + j;
                    let quad = &mut indices[(quad_index * 6)..(quad_index * 6 + 6)];
                    let vertex_index = (index_offset + (i * num_side_vertices + j)) as u32;
                    let next_row_vertex_index = vertex_index + num_side_vertices as u32;
                    quad.copy_from_slice(&[
                        vertex_index,
                        vertex_index + 1,
                        next_row_vertex_index,
                        next_row_vertex_index + 1,
                        next_row_vertex_index,
                        vertex_index + 1,
                    ]);
                }
            }
            (vertices, indices)
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for &(origin, u, v, color) in FACES_BASES {
            let (face_vertices, face_indices) = build_face(origin, u, v, color, vertices.len());
            vertices.extend(face_vertices.into_iter());
            indices.extend(face_indices.into_iter());
        }
        Self { vertices, indices }
    }
}

impl From<shape::Cube> for Mesh {
    fn from(value: shape::Cube) -> Self {
        let mut mesh = Mesh::unit_cube_subdivided(0);
        for vert in &mut mesh.vertices {
            vert.pos = value.side * vert.pos;
            vert.uv = value.side * vert.uv;
        }
        mesh
    }
}

impl From<shape::Sphere> for Mesh {
    fn from(value: shape::Sphere) -> Self {
        let mut mesh = Mesh::unit_cube_subdivided(5);
        for vert in &mut mesh.vertices {
            vert.pos = value.radius * vert.pos.norm();
            vert.uv = value.radius * vert.uv;
        }
        mesh
    }
}

impl From<shape::Box> for Mesh {
    fn from(value: shape::Box) -> Self {
        let diag = Vector3::new(value.width, value.depth, value.height);
        let faces_uv_scale = [
            Vector2::new(value.width, value.depth),
            Vector2::new(value.height, value.width),
            Vector2::new(value.depth, value.height),
            Vector2::new(value.depth, value.width),
            Vector2::new(value.width, value.height),
            Vector2::new(value.height, value.depth),
        ];
        let mut mesh = Mesh::unit_cube_subdivided(0);
        for (chunk, uv_scale) in &mut mesh.vertices.chunks_mut(4).zip(faces_uv_scale) {
            for vert in chunk {
                vert.pos = Vector3::new(
                    vert.pos.x * diag.x,
                    vert.pos.y * diag.y,
                    vert.pos.z * diag.z,
                );
                vert.uv = Vector2::new(vert.uv.x * uv_scale.x, vert.uv.y * uv_scale.y)
            }
        }
        mesh
    }
}

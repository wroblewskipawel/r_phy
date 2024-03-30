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

pub struct MeshBuilder {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
}

pub struct Mesh {
    pub(crate) vertices: Box<[Vertex]>,
    pub(crate) indices: Box<[u32]>,
}

impl MeshBuilder {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn build(self) -> Mesh {
        let Self { vertices, indices } = self;
        Mesh {
            vertices: vertices.into_boxed_slice(),
            indices: indices.into_boxed_slice(),
        }
    }

    fn extend(mut self, mut value: Self) -> Self {
        let index_offest = self.vertices.len() as u32;
        for index in &mut value.indices {
            *index += index_offest;
        }
        self.indices.extend(&value.indices);
        self.vertices.extend(&value.vertices);
        self
    }

    fn offset(mut self, offset: Vector3) -> Self {
        for vert in &mut self.vertices {
            vert.pos = vert.pos + offset;
        }
        self
    }

    fn plane_subdivided(
        num_subdiv: usize,
        u: Vector3,
        v: Vector3,
        color: Vector3,
        scale_uvs: bool,
    ) -> Self {
        let normal = u.cross(v).norm();
        let u_length = scale_uvs.then_some(u.length()).unwrap_or(1.0);
        let v_length = scale_uvs.then_some(v.length()).unwrap_or(1.0);
        let num_edge_vertices = 2 + num_subdiv;
        let num_vertices = num_edge_vertices.pow(2);
        let vertices = (0..num_vertices)
            .map(|index| (index / num_edge_vertices, index % num_edge_vertices))
            .map(|(i, j)| {
                let u_scale = j as f32 / (num_edge_vertices - 1) as f32;
                let v_scale = i as f32 / (num_edge_vertices - 1) as f32;
                Vertex {
                    pos: u_scale * u + v_scale * v,
                    color: color,
                    norm: normal,
                    uv: scale_uvs
                        .then_some(Vector2::new(u_scale * u_length, v_scale * v_length))
                        .unwrap_or(Vector2::new(u_scale, v_scale)),
                }
            })
            .collect();

        let num_edge_quads = 1 + num_subdiv;
        let num_quads = num_edge_quads.pow(2);
        let indices = (0..num_quads)
            .map(|index| (index / num_edge_quads, index % num_edge_quads))
            .flat_map(|(i, j)| {
                let vertex_index = (i * num_edge_vertices + j) as u32;
                let next_row_vertex_index = vertex_index + num_edge_vertices as u32;
                [
                    vertex_index,
                    vertex_index + 1,
                    next_row_vertex_index,
                    next_row_vertex_index + 1,
                    next_row_vertex_index,
                    vertex_index + 1,
                ]
            })
            .collect::<Vec<_>>();
        Self { vertices, indices }
    }

    fn box_subdivided(num_subdiv: usize, extent: Vector3, scale_uvs: bool) -> Self {
        const FACES: &[(Vector3, Vector3, Vector3, Vector3)] = &[
            (
                Vector3::new(0.5, 0.5, -0.5),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            (
                Vector3::new(-0.5, -0.5, -0.5),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 1.0, 0.0),
            ),
            (
                Vector3::new(-0.5, 0.5, -0.5),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
            (
                Vector3::new(-0.5, 0.5, 0.5),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            (
                Vector3::new(0.5, 0.5, -0.5),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 1.0, 0.0),
            ),
            (
                Vector3::new(0.5, -0.5, -0.5),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
        ];
        FACES
            .iter()
            .map(|&(offset, u, v, color)| {
                Self::plane_subdivided(
                    num_subdiv,
                    u.hadamard(extent),
                    v.hadamard(extent),
                    color,
                    scale_uvs,
                )
                .offset(offset.hadamard(extent))
            })
            .fold(Self::new(), |builder, face| builder.extend(face))
    }
}

impl From<shape::Cube> for Mesh {
    fn from(value: shape::Cube) -> Self {
        MeshBuilder::box_subdivided(0, Vector3::new(value.side, value.side, value.side), true)
            .build()
    }
}

impl From<shape::Sphere> for Mesh {
    fn from(value: shape::Sphere) -> Self {
        const UNIT_SPHERE_SUBDIV: usize = 4;
        let num_subdiv =
            ((value.diameter * UNIT_SPHERE_SUBDIV as f32) as usize).max(UNIT_SPHERE_SUBDIV);
        let mut mesh = MeshBuilder::box_subdivided(num_subdiv, Vector3::new(1.0, 1.0, 1.0), false);
        for vert in &mut mesh.vertices {
            vert.pos = 0.5 * value.diameter * vert.pos.norm();
            vert.uv = value.diameter * vert.uv;
        }
        mesh.build()
    }
}

impl From<shape::Box> for Mesh {
    fn from(value: shape::Box) -> Self {
        MeshBuilder::box_subdivided(
            0,
            Vector3::new(value.width, value.depth, value.height),
            true,
        )
        .build()
    }
}

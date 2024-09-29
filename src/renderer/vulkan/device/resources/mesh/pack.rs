use std::{any::TypeId, error::Error, marker::PhantomData};

use ash::vk;

use crate::renderer::{
    model::{Mesh, Vertex},
    vulkan::device::{
        command::operation::{self, Operation},
        memory::{AllocReqRaw, Allocator},
        resources::{
            buffer::{Buffer, BufferBuilder, BufferInfo, Range, StagingBufferBuilder},
            FromPartial, Partial, PartialBuilder,
        },
        VulkanDevice,
    },
};

use super::{
    BufferRanges, BufferType, MeshByteRange, MeshPackBinding, MeshPackData, MeshPackDataPartial,
};

pub struct MeshPackPartial<'a, V: Vertex> {
    partial: MeshPackDataPartial<'a, V>,
}

// TODO: Define trait for querrying for memory requirements
impl<'a, V: Vertex> MeshPackPartial<'a, V> {
    pub fn get_alloc_req_raw(&self) -> impl Iterator<Item = AllocReqRaw> {
        [self.partial.buffer.requirements().into()].into_iter()
    }
}

#[derive(Debug)]
pub struct MeshPack<V: Vertex, A: Allocator> {
    pub data: MeshPackData<A>,
    _phantom: PhantomData<V>,
}

#[derive(Debug)]
pub struct MeshPackRef<'a, V: Vertex, A: Allocator> {
    pub data: &'a MeshPackData<A>,
    pub _phantom: PhantomData<V>,
}

impl<'a, V: Vertex, A: Allocator> Clone for MeshPackRef<'a, V, A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, V: Vertex, A: Allocator> Copy for MeshPackRef<'a, V, A> {}

impl<'a, V: Vertex, T: Vertex, A: Allocator> TryFrom<&'a MeshPack<V, A>> for MeshPackRef<'a, T, A> {
    type Error = &'static str;

    fn try_from(value: &'a MeshPack<V, A>) -> Result<Self, Self::Error> {
        if TypeId::of::<T>() == TypeId::of::<V>() {
            Ok(Self {
                data: &value.data,
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid Vertex type")
        }
    }
}

impl<'a, V: Vertex, A: Allocator> From<MeshPackRef<'a, V, A>> for MeshPackBinding {
    fn from(value: MeshPackRef<'a, V, A>) -> Self {
        MeshPackBinding {
            buffer: value.data.buffer.handle(),
            buffer_ranges: value.data.buffer_ranges,
        }
    }
}

impl<'a, V: Vertex, A: Allocator> MeshPackRef<'a, V, A> {
    pub fn get(&self, index: usize) -> MeshRange<V> {
        MeshRange {
            vertices: self.data.meshes[index].vertices.into(),
            indices: self.data.meshes[index].indices.into(),
        }
    }

    pub fn as_raw(&self) -> &MeshPackData<A> {
        self.data
    }
}

impl<'a, V: Vertex, A: Allocator> From<&'a MeshPack<V, A>> for &'a MeshPackData<A> {
    fn from(value: &'a MeshPack<V, A>) -> Self {
        &value.data
    }
}

impl<'a, V: Vertex, A: Allocator> From<&'a mut MeshPack<V, A>> for &'a mut MeshPackData<A> {
    fn from(value: &'a mut MeshPack<V, A>) -> Self {
        &mut value.data
    }
}

impl<'a, V: Vertex, A: Allocator> From<&'a MeshPack<V, A>> for MeshPackBinding {
    fn from(value: &'a MeshPack<V, A>) -> Self {
        (&value.data).into()
    }
}

impl<V: Vertex, A: Allocator> MeshPack<V, A> {
    pub fn get(&self, index: usize) -> MeshRange<V> {
        self.data.meshes[index].into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshRangeBindData {
    pub index_count: u32,
    pub index_offset: u32,
    pub vertex_offset: i32,
}

impl<V: Vertex> From<MeshRange<V>> for MeshRangeBindData {
    fn from(value: MeshRange<V>) -> Self {
        MeshRangeBindData {
            index_count: value.indices.len as u32,
            index_offset: value.indices.first as u32,
            vertex_offset: value.vertices.first as i32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshRange<V: Vertex> {
    pub vertices: Range<V>,
    pub indices: Range<u32>,
}

impl VulkanDevice {
    // TODO: Should &self be &mut? Consider renaming the function to create_mesh_pack
    pub fn prepare_mesh_pack<'a, V: Vertex>(
        &self,
        meshes: &'a [Mesh<V>],
    ) -> Result<MeshPackPartial<'a, V>, Box<dyn Error>> {
        let num_vertices = meshes.iter().fold(0, |acc, mesh| acc + mesh.vertices.len());
        let num_indices = meshes.iter().fold(0, |acc, mesh| acc + mesh.indices.len());
        let mut builder = StagingBufferBuilder::new();
        let vertex_range = builder.append::<V>(num_vertices);
        let index_range = builder.append::<u32>(num_indices);
        let mut buffer_ranges = BufferRanges::new();
        buffer_ranges.set(BufferType::Vertex, vertex_range);
        buffer_ranges.set(BufferType::Index, index_range);
        let buffer = BufferBuilder::new(BufferInfo {
            size: buffer_ranges.get_rquired_buffer_size(),
            usage: vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::INDEX_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[operation::Graphics::get_queue_family_index(self)],
        })
        .prepare(self)?;
        let partial = MeshPackDataPartial {
            buffer,
            buffer_ranges,
            meshes,
        };
        Ok(MeshPackPartial { partial })
    }

    pub fn allocate_mesh_pack_memory<V: Vertex, A: Allocator>(
        &self,
        allocator: &mut A,
        partial: MeshPackPartial<V>,
    ) -> Result<MeshPack<V, A>, Box<dyn Error>> {
        let MeshPackPartial {
            partial:
                MeshPackDataPartial {
                    buffer,
                    buffer_ranges,
                    meshes,
                },
        } = partial;
        let mut buffer = Buffer::finalize(buffer, self, allocator)?;
        let num_indices = meshes.iter().fold(0, |acc, mesh| acc + mesh.indices.len());
        let num_vertices = meshes.iter().fold(0, |acc, mesh| acc + mesh.vertices.len());
        let mut builder = StagingBufferBuilder::new();
        let vertex_range = builder.append::<V>(num_vertices);
        let index_range = builder.append::<u32>(num_indices);
        let (vertex_ranges, index_ranges) = {
            let mut staging_buffer = self.create_stagging_buffer(builder)?;
            let mut vertex_writer = staging_buffer.write_range::<V>(vertex_range);
            let vertex_ranges = meshes
                .iter()
                .map(|mesh| vertex_writer.write(&mesh.vertices))
                .collect::<Vec<_>>();
            let mut index_writer = staging_buffer.write_range::<u32>(index_range);
            let index_ranges = meshes
                .iter()
                .map(|mesh| index_writer.write(&mesh.indices))
                .collect::<Vec<_>>();
            staging_buffer.transfer_buffer_data(&mut buffer, 0)?;
            (vertex_ranges, index_ranges)
        };
        let meshes = vertex_ranges
            .into_iter()
            .zip(index_ranges)
            .map(|(vertices, indices)| MeshByteRange {
                vertices: vertices.into(),
                indices: indices.into(),
            })
            .collect();
        let data = MeshPackData {
            buffer,
            buffer_ranges,
            meshes,
        };
        Ok(MeshPack {
            data,
            _phantom: PhantomData,
        })
    }

    pub fn load_mesh_pack<V: Vertex, A: Allocator>(
        &self,
        allocator: &mut A,
        meshes: &[Mesh<V>],
    ) -> Result<MeshPack<V, A>, Box<dyn Error>> {
        let mesh_pack = self.prepare_mesh_pack(meshes)?;
        let mesh_pack = self.allocate_mesh_pack_memory(allocator, mesh_pack)?;
        Ok(mesh_pack)
    }
}

use std::{any::TypeId, error::Error, marker::PhantomData};

use ash::vk;

use crate::renderer::{
    model::{Mesh, MeshHandle, Vertex},
    vulkan::device::{
        buffer::{Range, StagingBufferBuilder},
        command::operation::{self, Operation},
        VulkanDevice,
    },
};

use super::{
    BufferRanges, BufferType, MeshByteRange, MeshPackBinding, MeshPackData, VulkanMeshHandle,
};

#[derive(Debug)]
pub struct MeshPack<V: Vertex> {
    pub index: usize,
    pub data: MeshPackData,
    _phantom: PhantomData<V>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPackRef<'a, V: Vertex> {
    pub index: usize,
    pub data: &'a MeshPackData,
    pub _phantom: PhantomData<V>,
}

impl<'a, V: Vertex, T: Vertex> TryFrom<&'a MeshPack<V>> for MeshPackRef<'a, T> {
    type Error = &'static str;

    fn try_from(value: &'a MeshPack<V>) -> Result<Self, Self::Error> {
        if TypeId::of::<T>() == TypeId::of::<V>() {
            Ok(Self {
                index: value.index,
                data: &value.data,
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid Vertex type")
        }
    }
}

impl<'a, V: Vertex> From<MeshPackRef<'a, V>> for MeshPackBinding {
    fn from(value: MeshPackRef<'a, V>) -> Self {
        MeshPackBinding {
            buffer: value.data.buffer.buffer.buffer,
            buffer_ranges: value.data.buffer_ranges,
        }
    }
}

impl<'a, V: Vertex> MeshPackRef<'a, V> {
    // pub fn get_handles(&self) -> Vec<MeshHandle<V>> {
    //     self.data
    //         .meshes
    //         .iter()
    //         .enumerate()
    //         .map(|(mesh_index, _)| {
    //             VulkanMeshHandle::new(self.index as u32, mesh_index as u32).into()
    //         })
    //         .collect()
    // }

    pub fn get(&self, index: usize) -> MeshRange<V> {
        MeshRange {
            vertices: self.data.meshes[index].vertices.into(),
            indices: self.data.meshes[index].indices.into(),
        }
    }

    pub fn as_raw(&self) -> &MeshPackData {
        self.data
    }
}

impl<'a, V: Vertex> From<&'a MeshPack<V>> for &'a MeshPackData {
    fn from(value: &'a MeshPack<V>) -> Self {
        &value.data
    }
}

impl<'a, V: Vertex> From<&'a mut MeshPack<V>> for &'a mut MeshPackData {
    fn from(value: &'a mut MeshPack<V>) -> Self {
        &mut value.data
    }
}

impl<'a, V: Vertex> From<&'a MeshPack<V>> for MeshPackBinding {
    fn from(value: &'a MeshPack<V>) -> Self {
        (&value.data).into()
    }
}

impl<V: Vertex> MeshPack<V> {
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
    pub fn load_mesh_pack<V: Vertex>(
        &mut self,
        meshes: &[Mesh<V>],
        index: usize,
    ) -> Result<MeshPack<V>, Box<dyn Error>> {
        let num_vertices = meshes.iter().fold(0, |acc, mesh| acc + mesh.vertices.len());
        let num_indices = meshes.iter().fold(0, |acc, mesh| acc + mesh.indices.len());
        let mut builder = StagingBufferBuilder::new();
        let vertex_range = builder.append::<V>(num_vertices);
        let index_range = builder.append::<u32>(num_indices);
        let mut buffer_ranges = BufferRanges::new();
        buffer_ranges.set(BufferType::Vertex, vertex_range);
        buffer_ranges.set(BufferType::Index, index_range);
        let mut buffer = self.create_device_local_buffer(
            buffer_ranges.get_rquired_buffer_size(),
            vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::INDEX_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST,
            vk::SharingMode::EXCLUSIVE,
            &[operation::Graphics::get_queue_family_index(self)],
        )?;
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
            index,
            data,
            _phantom: PhantomData,
        })
    }
}

use std::{error::Error, ops::Index};

use ash::vk;
use strum::EnumCount;

use crate::renderer::model::{Mesh, Vertex};

use super::{
    buffer::{ByteRange, DeviceLocalBuffer, Range, StagingBufferBuilder},
    command::operation::{self, Operation},
    VulkanDevice,
};

#[derive(strum::EnumCount)]
pub enum BufferType {
    Vertex,
    Index,
}

pub struct BufferRanges {
    ranges: [Option<ByteRange>; BufferType::COUNT],
}

impl Index<BufferType> for BufferRanges {
    type Output = ByteRange;
    fn index(&self, index: BufferType) -> &Self::Output {
        self.ranges[index as usize]
            .as_ref()
            .expect("Required bufer data not present!")
    }
}

impl BufferRanges {
    fn new() -> Self {
        Self {
            ranges: [None; BufferType::COUNT],
        }
    }

    fn get_rquired_buffer_size(&self) -> usize {
        self.ranges
            .iter()
            .filter_map(|&range| range)
            .max_by_key(|range| range.end)
            .unwrap()
            .end
    }

    fn set(&mut self, buffer_type: BufferType, range: impl Into<ByteRange>) {
        self.ranges[buffer_type as usize] = Some(range.into());
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshRange {
    pub vertices: Range<Vertex>,
    pub indices: Range<u32>,
}

pub struct MeshPack {
    pub buffer: DeviceLocalBuffer,
    pub buffer_ranges: BufferRanges,
    pub meshes: Vec<MeshRange>,
}

impl VulkanDevice {
    // TODO: Should &self be &mut? Consider renaming the function to create_mesh_pack
    pub fn load_mesh_pack(&self, meshes: &[Mesh]) -> Result<MeshPack, Box<dyn Error>> {
        let num_vertices = meshes.iter().fold(0, |acc, mesh| acc + mesh.vertices.len());
        let num_indices = meshes.iter().fold(0, |acc, mesh| acc + mesh.indices.len());
        let mut builder = StagingBufferBuilder::new();
        let vertex_range = builder.append::<Vertex>(num_vertices);
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
            let mut vertex_writer = staging_buffer.write_range::<Vertex>(vertex_range);
            let vertex_ranges = meshes
                .iter()
                .map(|mesh| vertex_writer.write(&mesh.vertices))
                .collect::<Vec<Range<_>>>();
            let mut index_writer = staging_buffer.write_range::<u32>(index_range);
            let index_ranges = meshes
                .iter()
                .map(|mesh| index_writer.write(&mesh.indices))
                .collect::<Vec<Range<_>>>();
            staging_buffer.transfer_buffer_data(&mut buffer, 0)?;
            (vertex_ranges, index_ranges)
        };
        let meshes = vertex_ranges
            .into_iter()
            .zip(index_ranges)
            .map(|(vertices, indices)| MeshRange { vertices, indices })
            .collect();
        Ok(MeshPack {
            buffer,
            buffer_ranges,
            meshes,
        })
    }

    pub fn destroy_mesh_pack(&self, pack: &mut MeshPack) {
        self.destroy_buffer((&mut pack.buffer).into());
    }
}

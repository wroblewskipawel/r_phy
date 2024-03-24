use std::{
    error::Error,
    mem::{size_of, size_of_val},
    ops::Index,
};

use ash::vk;
use bytemuck::Pod;
use strum::EnumCount;

use crate::renderer::mesh::{Mesh, Vertex};

use super::{
    buffer::{DeviceLocalBuffer, Range},
    command::operation::{self, Operation},
    VulkanDevice,
};

#[derive(strum::EnumCount)]
pub enum BufferType {
    Vertex,
    Index,
}

#[repr(C)]
pub struct BufferRanges {
    ranges: [Option<Range>; BufferType::COUNT],
}

impl Index<BufferType> for BufferRanges {
    type Output = Range;
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
            .max_by_key(|range| range.offset)
            .map(|range| range.offset + range.size)
            .unwrap() as usize
    }

    fn set(&mut self, buffer_type: BufferType, range: Range) {
        self.ranges[buffer_type as usize] = Some(range);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Elements {
    pub first: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshRange {
    pub vertices: Elements,
    pub indices: Elements,
}

pub struct ResourcePack {
    pub buffer: DeviceLocalBuffer,
    pub buffer_ranges: BufferRanges,
    pub meshes: Vec<MeshRange>,
}

impl VulkanDevice {
    pub fn load_resource_pack(&mut self, meshes: &[Mesh]) -> Result<ResourcePack, Box<dyn Error>> {
        let buffer_ranges = Self::get_buffer_ranges(meshes);
        let buffer = self.create_device_local_buffer(
            buffer_ranges.get_rquired_buffer_size(),
            vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::INDEX_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST,
            vk::SharingMode::EXCLUSIVE,
            &[operation::Graphics::get_queue_family_index(self)],
        )?;
        let (vertex_ranges, index_ranges) = {
            let mut staging_buffer = self.create_stagging_buffer(buffer.buffer.size)?;
            let (_, vertex_ranges) = staging_buffer.load_buffer_data_from_slices(
                &meshes
                    .iter()
                    .map(|mesh| mesh.vertices.as_slice())
                    .collect::<Vec<_>>(),
                size_of::<f32>(),
            )?;
            let (_, index_ranges) = staging_buffer.load_buffer_data_from_slices(
                &meshes
                    .iter()
                    .map(|mesh| mesh.indices.as_slice())
                    .collect::<Vec<_>>(),
                size_of::<u32>(),
            )?;
            staging_buffer.transfer_data(&buffer, 0)?;
            (vertex_ranges, index_ranges)
        };

        let index_buffer_offset = buffer_ranges[BufferType::Index].offset;
        let meshes = vertex_ranges
            .into_iter()
            .zip(index_ranges)
            .map(|(vertices, indices)| MeshRange {
                vertices: Elements {
                    first: (vertices.offset / size_of::<Vertex>() as u64) as u32,
                    count: (vertices.size / size_of::<Vertex>() as u64) as u32,
                },
                indices: Elements {
                    first: ((indices.offset - index_buffer_offset) / size_of::<u32>() as u64)
                        as u32,
                    count: (indices.size / size_of::<u32>() as u64) as u32,
                },
            })
            .collect();
        Ok(ResourcePack {
            buffer,
            buffer_ranges,
            meshes,
        })
    }

    pub fn destory_resource_pack(&self, resources: &mut ResourcePack) {
        self.destroy_buffer((&mut resources.buffer).into());
    }

    fn get_buffer_ranges(meshes: &[Mesh]) -> BufferRanges {
        let vertex_data_size =
            Self::get_required_buffer_size(meshes.iter().map(|mesh| mesh.vertices.as_slice()));
        let index_data_size =
            Self::get_required_buffer_size(meshes.iter().map(|mesh| mesh.indices.as_slice()));
        let index_buffer_offset = Self::get_offset_aligned(vertex_data_size, size_of::<u32>());
        let mut ranges = BufferRanges::new();
        ranges.set(
            BufferType::Vertex,
            Range {
                offset: 0,
                size: vertex_data_size as vk::DeviceSize,
            },
        );
        ranges.set(
            BufferType::Index,
            Range {
                offset: index_buffer_offset as vk::DeviceSize,
                size: index_data_size as vk::DeviceSize,
            },
        );
        ranges
    }

    fn get_offset_aligned(offset: usize, alignment: usize) -> usize {
        ((offset + (alignment - 1)) / alignment) * alignment
    }

    fn get_required_buffer_size<'a, T: Pod>(slices: impl Iterator<Item = &'a [T]>) -> usize {
        slices.map(size_of_val).sum()
    }
}

use std::{error::Error, mem::size_of, ops::Index};

use ash::vk;
use bytemuck::{cast_slice, Pod};
use strum::EnumCount;

use crate::renderer::{mesh::Mesh, vulkan::device::Operation};

use super::{
    buffer::{Buffer, Range},

    VulkanDevice,
};

#[derive(strum::EnumCount)]
enum BufferType {
    Vertex,
    Index,
}

#[repr(C)]
struct BufferRanges {
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

pub struct Elements {
    first: u32,
    count: u32,
}

pub struct MeshRange {
    vertices: Elements,
    indices: Elements,
}

pub struct ResourcePack {
    buffer: Buffer,
    buffer_ranges: BufferRanges,
    pub meshes: Vec<MeshRange>,
}

impl VulkanDevice {
    pub fn load_resource_pack(&mut self, meshes: &[Mesh]) -> Result<ResourcePack, Box<dyn Error>> {
        let buffer_ranges = Self::get_buffer_ranges(meshes);
        let mut buffer = self.create_buffer(
            buffer_ranges.get_rquired_buffer_size(),
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            &self.get_queue_families(&[Operation::Graphics]),
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;
        let vertex_data_ranges = self.load_buffer_data_from_slices(
            buffer_ranges[BufferType::Vertex],
            &mut buffer,
            meshes.iter().map(|mesh| mesh.vertices.as_slice()),
        )?;
        let index_data_ranges = self.load_buffer_data_from_slices(
            buffer_ranges[BufferType::Index],
            &mut buffer,
            meshes.iter().map(|mesh| mesh.indices.as_slice()),
        )?;
        let meshes = vertex_data_ranges
            .into_iter()
            .zip(index_data_ranges.into_iter())
            .map(|(vertices, indices)| MeshRange { vertices, indices })
            .collect();
        Ok(ResourcePack {
            buffer,
            buffer_ranges,
            meshes,
        })
    }

    pub fn use_resource_pack(&self, command_buffer: vk::CommandBuffer, resources: &ResourcePack) {
        unsafe {
            self.device.cmd_bind_index_buffer(
                command_buffer,
                resources.buffer.buffer,
                resources.buffer_ranges[BufferType::Index].offset,
                vk::IndexType::UINT32,
            );
            self.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[resources.buffer.buffer],
                &[resources.buffer_ranges[BufferType::Vertex].offset],
            );
        }
    }

    pub fn draw(
        &self,
        command_buffer: vk::CommandBuffer,
        resources: &ResourcePack,
        mesh_index: usize,
    ) {
        let mesh_ranges = &resources.meshes[mesh_index];
        unsafe {
            self.device.cmd_draw_indexed(
                command_buffer,
                mesh_ranges.indices.count as u32,
                1,
                mesh_ranges.indices.first as u32,
                mesh_ranges.vertices.first as i32,
                0,
            )
        }
    }

    pub fn destory_resource_pack(&self, resources: &mut ResourcePack) {
        self.destroy_buffer(&mut resources.buffer);
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
        slices.map(|slice| slice.len() * size_of::<T>()).sum()
    }

    fn load_buffer_data_from_slices<'a, T: Pod>(
        &self,
        dst_range: Range,
        dst_buffer: &mut Buffer,
        src_slices: impl Iterator<Item = &'a [T]>,
    ) -> Result<Vec<Elements>, Box<dyn Error>> {
        let mut buffer = self.map_buffer_range(dst_buffer, dst_range)?;
        let mut buffer_offset = 0;
        let mut element_offset = 0;
        let mut slice_ranges = vec![];
        for slice in src_slices {
            let bytes: &[u8] = cast_slice(slice);
            let num_elements = slice.len() as u32;
            buffer.copy_data(buffer_offset, bytes);
            slice_ranges.push(Elements {
                first: element_offset,
                count: num_elements,
            });
            element_offset += num_elements;
            buffer_offset += bytes.len();
        }
        Ok(slice_ranges)
    }
}

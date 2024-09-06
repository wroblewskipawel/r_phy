mod list;
mod pack;

use ash::vk;
pub use list::*;
pub use pack::*;

use std::ops::Index;

use strum::EnumCount;

use crate::renderer::model::Vertex;
use crate::renderer::vulkan::device::buffer::Buffer;
use crate::renderer::vulkan::device::{
    buffer::{ByteRange, DeviceLocalBuffer},
    VulkanDevice,
};

#[derive(strum::EnumCount)]
pub enum BufferType {
    Vertex,
    Index,
}

#[derive(Debug, Clone, Copy)]
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
pub struct MeshByteRange {
    pub vertices: ByteRange,
    pub indices: ByteRange,
}

impl<V: Vertex> From<MeshByteRange> for MeshRange<V> {
    fn from(value: MeshByteRange) -> Self {
        Self {
            vertices: value.vertices.into(),
            indices: value.indices.into(),
        }
    }
}

#[derive(Debug)]
pub struct MeshPackData {
    pub buffer: DeviceLocalBuffer,
    pub buffer_ranges: BufferRanges,
    pub meshes: Vec<MeshByteRange>,
}

impl<'a> From<&'a mut MeshPackData> for &'a mut Buffer {
    fn from(value: &'a mut MeshPackData) -> Self {
        (&mut value.buffer).into()
    }
}

impl VulkanDevice {
    pub fn destroy_mesh_pack<'a>(&self, pack: impl Into<&'a mut MeshPackData>) {
        self.destroy_buffer((&mut pack.into().buffer).into());
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPackBinding {
    pub buffer: vk::Buffer,
    pub buffer_ranges: BufferRanges,
}

impl<'a> From<&'a MeshPackData> for MeshPackBinding {
    fn from(value: &'a MeshPackData) -> Self {
        Self {
            // TODO: Improve buffer aggregation scheme and naming
            buffer: value.buffer.buffer.buffer,
            buffer_ranges: value.buffer_ranges,
        }
    }
}

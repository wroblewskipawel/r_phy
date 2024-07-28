mod list;
mod type_erased;
mod type_safe;

pub use list::*;
pub use type_erased::*;
pub use type_safe::*;

use std::ops::Index;

use strum::EnumCount;

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
pub struct MeshPackData {
    pub buffer: DeviceLocalBuffer,
    pub buffer_ranges: BufferRanges,
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

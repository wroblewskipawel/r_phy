mod list;
mod pack;

use ash::vk;
pub use list::*;
pub use pack::*;

use std::ops::Index;

use strum::EnumCount;

use to_resolve::model::{Mesh, Vertex};

use crate::device::memory::{Allocator, DeviceLocal};

use super::buffer::{Buffer, BufferPartial, ByteRange};

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

pub struct MeshPackDataPartial<'a, V: Vertex> {
    meshes: &'a [Mesh<V>],
    buffer_ranges: BufferRanges,
    buffer: BufferPartial<DeviceLocal>,
}

#[derive(Debug)]
pub struct MeshPackData<A: Allocator> {
    buffer: Buffer<DeviceLocal, A>,
    buffer_ranges: BufferRanges,
    meshes: Vec<MeshByteRange>,
}

impl<'a, A: Allocator> From<&'a mut MeshPackData<A>> for &'a mut Buffer<DeviceLocal, A> {
    fn from(value: &'a mut MeshPackData<A>) -> Self {
        (&mut value.buffer).into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPackBinding {
    pub buffer: vk::Buffer,
    pub buffer_ranges: BufferRanges,
}

impl<'a, A: Allocator> From<&'a MeshPackData<A>> for MeshPackBinding {
    fn from(value: &'a MeshPackData<A>) -> Self {
        Self {
            buffer: value.buffer.handle(),
            buffer_ranges: value.buffer_ranges,
        }
    }
}

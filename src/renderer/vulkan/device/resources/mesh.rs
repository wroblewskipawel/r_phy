use std::any::TypeId;
use std::marker::PhantomData;
use std::{error::Error, ops::Index};

use ash::vk;
use strum::EnumCount;

use crate::renderer::model::{
    Mesh, MeshCollection, MeshHandle, MeshList, MeshNode, MeshTerminator, Vertex,
};

use crate::renderer::vulkan::device::{
    buffer::{ByteRange, DeviceLocalBuffer, Range, StagingBufferBuilder},
    command::operation::{self, Operation},
    VulkanDevice,
};

#[derive(strum::EnumCount)]
pub enum BufferType {
    Vertex,
    Index,
}

#[derive(Debug)]
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
pub struct MeshRangeRaw {
    pub vertices: ByteRange,
    pub indices: ByteRange,
}

#[derive(Debug)]
pub struct MeshPackRaw {
    index: usize,
    pub buffer: DeviceLocalBuffer,
    pub buffer_ranges: BufferRanges,
    pub meshes: Vec<MeshRangeRaw>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPack<'a, V: Vertex> {
    // TODO: Remove
    index: usize,
    raw: &'a MeshPackRaw,
    _phatnom: PhantomData<V>,
}

impl<'a, V: Vertex> From<&'a MeshPackRaw> for MeshPack<'a, V> {
    fn from(value: &'a MeshPackRaw) -> Self {
        Self {
            index: value.index,
            raw: value,
            _phatnom: PhantomData,
        }
    }
}

impl<'a, V: Vertex> From<MeshPack<'a, V>> for &'a MeshPackRaw {
    fn from(value: MeshPack<'a, V>) -> Self {
        value.raw
    }
}

impl<'a, V: Vertex> MeshPack<'a, V> {
    pub fn get_handles(&self) -> Vec<MeshHandle<V>> {
        self.raw
            .meshes
            .iter()
            .enumerate()
            .map(|(mesh_index, _)| {
                VulkanMeshHandle {
                    mesh_pack_index: self.index as u32,
                    mesh_index: mesh_index as u32,
                    _phantom: PhantomData,
                }
                .into()
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshRange<V: Vertex> {
    pub vertices: Range<V>,
    pub indices: Range<u32>,
}

impl<V: Vertex> From<MeshRangeRaw> for MeshRange<V> {
    fn from(value: MeshRangeRaw) -> Self {
        Self {
            vertices: value.vertices.into(),
            indices: value.indices.into(),
        }
    }
}

impl<'a, V: Vertex> MeshPack<'a, V> {
    pub fn get(&self, index: usize) -> MeshRange<V> {
        self.raw.meshes[index].into()
    }
}

impl VulkanDevice {
    // TODO: Should &self be &mut? Consider renaming the function to create_mesh_pack
    pub fn load_mesh_pack<V: Vertex>(
        &self,
        meshes: &[Mesh<V>],
        index: usize,
    ) -> Result<MeshPackRaw, Box<dyn Error>> {
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
                .map(|mesh| vertex_writer.write(&mesh.vertices).into())
                .collect::<Vec<ByteRange>>();
            let mut index_writer = staging_buffer.write_range::<u32>(index_range);
            let index_ranges = meshes
                .iter()
                .map(|mesh| index_writer.write(&mesh.indices).into())
                .collect::<Vec<ByteRange>>();
            staging_buffer.transfer_buffer_data(&mut buffer, 0)?;
            (vertex_ranges, index_ranges)
        };
        let meshes = vertex_ranges
            .into_iter()
            .zip(index_ranges)
            .map(|(vertices, indices)| MeshRangeRaw { vertices, indices })
            .collect();
        Ok(MeshPackRaw {
            index,
            buffer,
            buffer_ranges,
            meshes,
        })
    }

    pub fn destroy_mesh_pack(&self, pack: &mut MeshPackRaw) {
        self.destroy_buffer((&mut pack.buffer).into());
    }
}

pub trait MeshPackList: MeshList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<'a, V: Vertex>(&'a self) -> Option<MeshPack<'a, V>>;
}

impl MeshPackList for MeshTerminator {
    fn destroy(&mut self, _device: &VulkanDevice) {}
    fn try_get<'a, V: Vertex>(&'a self) -> Option<MeshPack<'a, V>> {
        None
    }
}

pub struct MeshPackNode<V: Vertex, N: MeshPackList> {
    pub mesh_pack: MeshPackRaw,
    pub next: N,
    _phantom: PhantomData<V>,
}

impl<V: Vertex, N: MeshPackList> MeshList for MeshPackNode<V, N> {
    const LEN: usize = N::LEN + 1;
    type Vertex = V;
    type Next = N;
}

impl<V: Vertex, N: MeshPackList> MeshPackList for MeshPackNode<V, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        device.destroy_mesh_pack(&mut self.mesh_pack);
        self.next.destroy(device);
    }

    fn try_get<'a, T: Vertex>(&'a self) -> Option<MeshPack<'a, T>> {
        if TypeId::of::<V>() == TypeId::of::<T>() {
            Some((&self.mesh_pack).into())
        } else {
            self.next.try_get::<T>()
        }
    }
}

pub trait MeshPackListBuilder: MeshList {
    type Pack: MeshPackList;

    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl MeshPackListBuilder for MeshTerminator {
    type Pack = Self;

    fn build(&self, _device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Self {})
    }
}

impl<V: Vertex, N: MeshPackListBuilder> MeshPackListBuilder for MeshNode<V, N> {
    type Pack = MeshPackNode<V, N::Pack>;

    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(MeshPackNode {
            mesh_pack: device.load_mesh_pack(self.get(), Self::LEN)?,
            next: self.next().build(device)?,
            _phantom: PhantomData,
        })
    }
}

pub struct MeshPacks<L: MeshPackList> {
    pub packs: L,
}

impl VulkanDevice {
    pub fn load_meshes<B: MeshPackListBuilder>(
        &self,
        meshes: &B,
    ) -> Result<MeshPacks<B::Pack>, Box<dyn Error>> {
        Ok(MeshPacks {
            packs: meshes.build(self)?,
        })
    }

    pub fn destroy_meshes<M: MeshPackList>(&self, packs: &mut MeshPacks<M>) {
        packs.packs.destroy(self);
    }
}

pub struct VulkanMeshHandle<V: Vertex> {
    pub mesh_pack_index: u32,
    pub mesh_index: u32,
    _phantom: PhantomData<V>,
}

impl<V: Vertex> From<MeshHandle<V>> for VulkanMeshHandle<V> {
    fn from(value: MeshHandle<V>) -> Self {
        Self {
            mesh_pack_index: ((0xFFFFFFF0000000 & value.0) >> 32) as u32,
            mesh_index: (0x00000000FFFFFFFF & value.0) as u32,
            _phantom: PhantomData,
        }
    }
}

impl<V: Vertex> From<VulkanMeshHandle<V>> for MeshHandle<V> {
    fn from(value: VulkanMeshHandle<V>) -> Self {
        Self(
            ((value.mesh_pack_index as u64) << 32) + value.mesh_index as u64,
            PhantomData,
        )
    }
}

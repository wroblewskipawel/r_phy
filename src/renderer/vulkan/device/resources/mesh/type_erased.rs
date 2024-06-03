use std::{any::TypeId, marker::PhantomData};

use crate::renderer::{
    model::{MeshHandle, Vertex},
    vulkan::device::buffer::ByteRange,
};

use super::{MeshPack, MeshPackData, MeshRange, VulkanMeshHandle};

#[derive(Debug, Clone, Copy)]
pub struct MeshRangeTypeErased {
    pub vertices: ByteRange,
    pub indices: ByteRange,
}

impl<V: Vertex> From<MeshRange<V>> for MeshRangeTypeErased {
    fn from(value: MeshRange<V>) -> Self {
        Self {
            vertices: value.vertices.into(),
            indices: value.indices.into(),
        }
    }
}

#[derive(Debug)]
pub struct MeshPackTypeErased {
    type_id: TypeId,
    pub index: usize,
    pub data: MeshPackData,
    pub meshes: Vec<MeshRangeTypeErased>,
}

impl<V: Vertex> From<MeshPack<V>> for MeshPackTypeErased {
    fn from(value: MeshPack<V>) -> Self {
        Self {
            type_id: TypeId::of::<V>(),
            index: value.index,
            data: value.data,
            meshes: value.meshes.iter().map(|&mesh| mesh.into()).collect(),
        }
    }
}

impl<'a> From<&'a MeshPackTypeErased> for &'a MeshPackData {
    fn from(value: &'a MeshPackTypeErased) -> Self {
        &value.data
    }
}

impl<'a> From<&'a mut MeshPackTypeErased> for &'a mut MeshPackData {
    fn from(value: &'a mut MeshPackTypeErased) -> Self {
        &mut value.data
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPackRef<'a, V: Vertex> {
    pack: &'a MeshPackTypeErased,
    _phatnom: PhantomData<V>,
}

impl<'a, V: Vertex> TryFrom<&'a MeshPackTypeErased> for MeshPackRef<'a, V> {
    type Error = &'static str;

    fn try_from(value: &'a MeshPackTypeErased) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<V>() {
            Ok(Self {
                pack: value,
                _phatnom: PhantomData,
            })
        } else {
            Err("Invalid Vertex type")
        }
    }
}

impl<'a, V: Vertex> From<MeshPackRef<'a, V>> for &'a MeshPackData {
    fn from(value: MeshPackRef<'a, V>) -> Self {
        &value.pack.data
    }
}

impl<'a, V: Vertex> MeshPackRef<'a, V> {
    pub fn get_handles(&self) -> Vec<MeshHandle<V>> {
        self.pack
            .meshes
            .iter()
            .enumerate()
            .map(|(mesh_index, _)| {
                VulkanMeshHandle::new(self.pack.index as u32, mesh_index as u32).into()
            })
            .collect()
    }

    pub fn get(&self, index: usize) -> MeshRange<V> {
        MeshRange {
            vertices: self.pack.meshes[index].vertices.into(),
            indices: self.pack.meshes[index].indices.into(),
        }
    }
}

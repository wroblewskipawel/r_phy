use std::{error::Error, marker::PhantomData};

use crate::{
    core::{Cons, Nil},
    renderer::{
        model::{Mesh, MeshCollection, MeshHandle, MeshList, Vertex},
        vulkan::device::VulkanDevice,
    },
};

use super::{MeshPack, MeshPackRef};

pub trait MeshPackList: MeshList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V>>;
}

impl MeshPackList for Nil {
    fn destroy(&mut self, _device: &VulkanDevice) {}
    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V>> {
        None
    }
}

impl<V: Vertex, N: MeshPackList> MeshList for Cons<MeshPack<V>, N> {
    const LEN: usize = N::LEN + 1;
    type Vertex = V;
    type Next = N;
}

impl<V: Vertex, N: MeshPackList> MeshPackList for Cons<MeshPack<V>, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        device.destroy_mesh_pack(&mut self.head);
        self.tail.destroy(device);
    }

    fn try_get<T: Vertex>(&self) -> Option<MeshPackRef<T>> {
        if let Ok(mesh_pack_ref) = (&self.head).try_into() {
            Some(mesh_pack_ref)
        } else {
            self.tail.try_get::<T>()
        }
    }
}

pub trait MeshPackListBuilder: MeshList {
    type Pack: MeshPackList;

    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl MeshPackListBuilder for Nil {
    type Pack = Self;

    fn build(&self, _device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Self {})
    }
}

impl<V: Vertex, N: MeshPackListBuilder> MeshPackListBuilder for Cons<Vec<Mesh<V>>, N> {
    type Pack = Cons<MeshPack<V>, N::Pack>;

    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Cons {
            head: device.load_mesh_pack(self.get(), Self::LEN)?.into(),
            tail: self.next().build(device)?,
        })
    }
}

pub struct MeshPacks<L: MeshPackList> {
    pub packs: L,
}

impl VulkanDevice {
    pub fn load_meshes<B: MeshPackListBuilder>(
        &mut self,
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

impl<V: Vertex> VulkanMeshHandle<V> {
    pub fn new(mesh_pack_index: u32, mesh_index: u32) -> Self {
        Self {
            mesh_pack_index,
            mesh_index,
            _phantom: PhantomData,
        }
    }
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

use std::error::Error;

use crate::{
    core::{Cons, Nil},
    renderer::{
        model::{Mesh, MeshCollection, MeshTypeList, Vertex},
        vulkan::device::VulkanDevice,
    },
};

use super::{MeshPack, MeshPackRef};

pub trait MeshPackList: MeshTypeList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V>>;
}

impl MeshPackList for Nil {
    fn destroy(&mut self, _device: &VulkanDevice) {}

    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V>> {
        None
    }
}

impl<V: Vertex, N: MeshPackList> MeshTypeList for Cons<Option<MeshPack<V>>, N> {
    const LEN: usize = N::LEN + 1;
    type Vertex = V;
    type Next = N;
}

impl<V: Vertex, N: MeshPackList> MeshPackList for Cons<Option<MeshPack<V>>, N> {
    fn destroy(&mut self, device: &VulkanDevice) {
        if let Some(mesh_pack) = &mut self.head {
            device.destroy_mesh_pack(mesh_pack);
        }
        self.tail.destroy(device);
    }

    fn try_get<T: Vertex>(&self) -> Option<MeshPackRef<T>> {
        self.head
            .as_ref()
            .and_then(|pack| pack.try_into().ok())
            .or_else(|| self.tail.try_get::<T>())
    }
}

pub trait MeshPackListBuilder: MeshTypeList {
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
    type Pack = Cons<Option<MeshPack<V>>, N::Pack>;

    fn build(&self, device: &mut VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        let meshes = self.get();
        let pack = if !meshes.is_empty() {
            Some(device.load_mesh_pack(self.get())?)
        } else {
            None
        };
        Ok(Cons {
            head: pack,
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

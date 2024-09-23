use std::error::Error;

use crate::{
    core::{Cons, Nil, TypedNil},
    renderer::{
        model::{Mesh, MeshCollection, MeshTypeList, Vertex},
        vulkan::device::{memory::Allocator, VulkanDevice},
    },
};

use super::{MeshPack, MeshPackPartial, MeshPackRef};

pub trait MeshPackList<A: Allocator>: MeshTypeList {
    fn destroy(&mut self, device: &VulkanDevice, allocator: &mut A);

    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V, A>>;
}

impl<A: Allocator> MeshPackList<A> for TypedNil<A> {
    fn destroy(&mut self, _device: &VulkanDevice, _allocator: &mut A) {}

    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V, A>> {
        None
    }
}

impl<V: Vertex, A: Allocator, N: MeshPackList<A>> MeshTypeList for Cons<Option<MeshPack<V, A>>, N> {
    const LEN: usize = N::LEN + 1;
    type Vertex = V;
    type Next = N;
}

impl<V: Vertex, A: Allocator, N: MeshPackList<A>> MeshPackList<A>
    for Cons<Option<MeshPack<V, A>>, N>
{
    fn destroy(&mut self, device: &VulkanDevice, allocator: &mut A) {
        if let Some(mesh_pack) = &mut self.head {
            device.destroy_mesh_pack(mesh_pack, allocator);
        }
        self.tail.destroy(device, allocator);
    }

    fn try_get<T: Vertex>(&self) -> Option<MeshPackRef<T, A>> {
        self.head
            .as_ref()
            .and_then(|pack| pack.try_into().ok())
            .or_else(|| self.tail.try_get::<T>())
    }
}

pub trait MeshPackListBuilder: MeshTypeList {
    type Pack<A: Allocator>: MeshPackList<A>;

    fn prepare<A: Allocator>(
        &self,
        device: &VulkanDevice,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>;
}

impl MeshPackListBuilder for Nil {
    type Pack<A: Allocator> = TypedNil<A>;

    fn prepare<A: Allocator>(
        &self,
        _device: &VulkanDevice,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        Ok(Nil {})
    }
}

impl<V: Vertex, N: MeshPackListBuilder> MeshPackListBuilder for Cons<Vec<Mesh<V>>, N> {
    type Pack<A: Allocator> = Cons<Option<MeshPack<V, A>>, N::Pack<A>>;

    fn prepare<A: Allocator>(
        &self,
        device: &VulkanDevice,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        let meshes = self.get();
        let partial = if !meshes.is_empty() {
            Some(device.prepare_mesh_pack(self.get())?)
        } else {
            None
        };
        Ok(Cons {
            head: partial,
            tail: self.tail.prepare(device)?,
        })
    }
}

pub trait MeshPackListPartial: Sized {
    type Pack<A: Allocator>: MeshPackList<A>;

    fn allocate<A: Allocator>(
        self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>;
}

impl MeshPackListPartial for Nil {
    type Pack<A: Allocator> = TypedNil<A>;

    fn allocate<A: Allocator>(
        self,
        _device: &VulkanDevice,
        _allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>> {
        Ok(TypedNil::new())
    }
}

impl<'a, V: Vertex, N: MeshPackListPartial> MeshPackListPartial
    for Cons<Option<MeshPackPartial<'a, V>>, N>
{
    type Pack<A: Allocator> = Cons<Option<MeshPack<V, A>>, N::Pack<A>>;

    fn allocate<A: Allocator>(
        self,
        device: &VulkanDevice,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>> {
        let Self { head, tail } = self;
        let pack = if let Some(partial) = head {
            Some(device.allocate_mesh_pack_memory(allocator, partial)?)
        } else {
            None
        };
        Ok(Cons {
            head: pack,
            tail: tail.allocate(device, allocator)?,
        })
    }
}

impl VulkanDevice {
    pub fn destroy_meshes<A: Allocator, M: MeshPackList<A>>(
        &self,
        packs: &mut M,
        allocator: &mut A,
    ) {
        packs.destroy(self, allocator);
    }
}

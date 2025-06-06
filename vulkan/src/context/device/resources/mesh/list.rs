use std::{cell::RefCell, error::Error};

use crate::context::device::{
    memory::{AllocReq, Allocator},
    resources::{DummyPack, PartialBuilder},
    Device,
};
use graphics::model::{Mesh, MeshTypeList, Vertex};
use type_kit::{Cons, Create, Destroy, Nil, TypedNil};

use super::{MeshPack, MeshPackPartial, MeshPackRef};

pub trait MeshPackList<A: Allocator>:
    for<'a> Destroy<Context<'a> = (&'a Device, &'a RefCell<&'a mut A>)>
{
    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V, A>>;
}

impl<A: Allocator> MeshPackList<A> for TypedNil<DummyPack<A>> {
    fn try_get<V: Vertex>(&self) -> Option<MeshPackRef<V, A>> {
        None
    }
}

impl<V: Vertex, A: Allocator, N: MeshPackList<A>> MeshPackList<A>
    for Cons<Option<MeshPack<V, A>>, N>
{
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
        device: &Device,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>;
}

impl MeshPackListBuilder for Nil {
    type Pack<A: Allocator> = TypedNil<DummyPack<A>>;

    fn prepare<A: Allocator>(
        &self,
        _device: &Device,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        Ok(Nil::new())
    }
}

impl<V: Vertex, N: MeshPackListBuilder> MeshPackListBuilder for Cons<Vec<Mesh<V>>, N> {
    type Pack<A: Allocator> = Cons<Option<MeshPack<V, A>>, N::Pack<A>>;

    fn prepare<A: Allocator>(
        &self,
        device: &Device,
    ) -> Result<impl MeshPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        let meshes = self.get();
        let partial = if !meshes.is_empty() {
            Some(MeshPackPartial::prepare(self.get(), device)?)
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

    fn get_memory_requirements(&self) -> Vec<AllocReq>;

    fn allocate<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>;
}

impl MeshPackListPartial for Nil {
    type Pack<A: Allocator> = TypedNil<DummyPack<A>>;

    fn get_memory_requirements(&self) -> Vec<AllocReq> {
        vec![]
    }

    fn allocate<A: Allocator>(
        self,
        _device: &Device,
        _allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>> {
        Ok(TypedNil::new())
    }
}

impl<'a, V: Vertex, N: MeshPackListPartial> MeshPackListPartial
    for Cons<Option<MeshPackPartial<'a, V>>, N>
{
    type Pack<A: Allocator> = Cons<Option<MeshPack<V, A>>, N::Pack<A>>;

    fn get_memory_requirements(&self) -> Vec<AllocReq> {
        let mut alloc_reqs = self.tail.get_memory_requirements();
        if let Some(partial) = &self.head {
            alloc_reqs.extend(partial.requirements());
        }
        alloc_reqs
    }

    fn allocate<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>> {
        let Self { head, tail } = self;
        let pack = if let Some(partial) = head {
            Some(MeshPack::create(
                partial,
                (device, &RefCell::new(allocator)),
            )?)
        } else {
            None
        };
        Ok(Cons {
            head: pack,
            tail: tail.allocate(device, allocator)?,
        })
    }
}

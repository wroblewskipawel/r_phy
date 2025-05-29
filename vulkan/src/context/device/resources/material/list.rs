use std::{cell::RefCell, error::Error};

use crate::context::device::{
    memory::{AllocReq, Allocator},
    resources::DummyPack,
    Device,
};
use graphics::model::{MaterialCollection, MaterialTypeList};
use type_kit::{Cons, Destroy, Nil, TypedNil};

use super::{Material, MaterialPack, MaterialPackPartial, MaterialPackRef};

pub trait MaterialPackListBuilder: MaterialTypeList {
    type Pack<A: Allocator>: MaterialPackList<A>;

    fn prepare<A: Allocator>(
        &self,
        device: &Device,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>>;
}

impl MaterialPackListBuilder for Nil {
    type Pack<A: Allocator> = TypedNil<DummyPack<A>>;

    fn prepare<A: Allocator>(
        &self,
        _device: &Device,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        Ok(Nil::new())
    }
}

impl<M: Material, N: MaterialPackListBuilder> MaterialPackListBuilder for Cons<Vec<M>, N> {
    type Pack<A: Allocator> = Cons<Option<MaterialPack<M, A>>, N::Pack<A>>;

    fn prepare<A: Allocator>(
        &self,
        device: &Device,
    ) -> Result<impl MaterialPackListPartial<Pack<A> = Self::Pack<A>>, Box<dyn Error>> {
        let materials = self.get();
        let partial = if !materials.is_empty() {
            Some(device.prepare_material_pack(materials)?)
        } else {
            None
        };
        Ok(Cons {
            head: partial,
            tail: self.next().prepare(device)?,
        })
    }
}

pub trait MaterialPackListPartial: Sized {
    type Pack<A: Allocator>: MaterialPackList<A>;

    fn get_memory_requirements(&self) -> Vec<AllocReq>;

    fn allocate<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>>;
}

impl MaterialPackListPartial for Nil {
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

impl<'a, M: Material, N: MaterialPackListPartial> MaterialPackListPartial
    for Cons<Option<MaterialPackPartial<'a, M>>, N>
{
    type Pack<A: Allocator> = Cons<Option<MaterialPack<M, A>>, N::Pack<A>>;

    fn get_memory_requirements(&self) -> Vec<AllocReq> {
        let mut alloc_reqs = self.tail.get_memory_requirements();
        if let Some(partial) = &self.head {
            alloc_reqs.extend(partial.get_alloc_req());
        }
        alloc_reqs
    }

    fn allocate<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Pack<A>, Box<dyn Error>> {
        let Self { head, tail } = self;
        let pack = if let Some(pack) = head {
            Some(device.allocate_material_pack_memory(allocator, pack)?)
        } else {
            None
        };
        Ok(Cons {
            head: pack,
            tail: tail.allocate(device, allocator)?,
        })
    }
}

pub trait MaterialPackList<A: Allocator>:
    for<'a> Destroy<Context<'a> = (&'a Device, &'a RefCell<&'a mut A>)>
{
    fn try_get<M: Material>(&self) -> Option<MaterialPackRef<M>>;
}

impl<A: Allocator> MaterialPackList<A> for TypedNil<DummyPack<A>> {
    fn try_get<T: Material>(&self) -> Option<MaterialPackRef<T>> {
        None
    }
}

impl<A: Allocator, M: Material, N: MaterialPackList<A>> MaterialPackList<A>
    for Cons<Option<MaterialPack<M, A>>, N>
{
    fn try_get<T: Material>(&self) -> Option<MaterialPackRef<T>> {
        self.head
            .as_ref()
            .and_then(|pack| pack.try_into().ok())
            .or_else(|| self.tail.try_get::<T>())
    }
}

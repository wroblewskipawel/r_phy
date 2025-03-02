pub mod buffer;
pub mod memory;

use std::convert::Infallible;

use buffer::BufferRaw;
use memory::MemoryRaw;
use type_kit::{
    list_type, Cons, Contains, Conv, Create, CreateResult, Destroy, DestroyResult, DropGuard,
    DropGuardError, FromGuard, GenIndexRaw, GuardCollection, GuardIndex, Marker, Nil,
    ScopedEntryMutResult, ScopedEntryResult, TypeGuard, TypedIndex, Valid,
};

use crate::{
    device::Device,
    error::{ResourceError, ResourceResult},
};

pub trait Resource:
    FromGuard<Inner = Self::RawType>
    + for<'a> Create<Context<'a> = &'a Device, CreateError = ResourceError>
{
    type RawType: Clone + Copy + for<'a> Destroy<Context<'a> = Self::Context<'a>>;
}

pub type Raw<R> = <R as Resource>::RawType;

#[derive(Debug, Clone, Copy)]
pub struct ResourceIndex<R: Resource> {
    index: GuardIndex<R>,
}

impl<R: Resource> FromGuard for ResourceIndex<R> {
    type Inner = GenIndexRaw;

    #[inline]
    fn into_inner(self) -> Self::Inner {
        self.index.into_inner()
    }
}

impl<R: Resource> From<Valid<ResourceIndex<R>>> for ResourceIndex<R> {
    fn from(value: Valid<ResourceIndex<R>>) -> Self {
        let index = unsafe { TypeGuard::from_inner::<GuardIndex<R>>(value.into_inner()) };
        let index: Conv<GuardIndex<R>> = index.try_into().unwrap();
        Self {
            index: index.unwrap(),
        }
    }
}

pub type RawCollection<R> = GuardCollection<<R as Resource>::RawType>;
pub type ResourceStorageList =
    list_type![GuardCollection<MemoryRaw>, GuardCollection<BufferRaw>, Nil];

#[derive(Debug)]
pub struct ResourceStorage {
    storage: ResourceStorageList,
}

impl ResourceStorage {
    #[inline]
    pub fn new() -> Self {
        ResourceStorage {
            storage: ResourceStorageList::default(),
        }
    }

    #[inline]
    pub fn create_resource<'a, R: Resource, M: Marker>(
        &mut self,
        device: &Device,
        config: R::Config<'a>,
    ) -> ResourceResult<ResourceIndex<R>>
    where
        ResourceStorageList: Contains<RawCollection<R>, M>,
    {
        let resource = R::create(config, device)?;
        let index = self.storage.get_mut().push(resource.into_guard())?;
        Ok(ResourceIndex { index })
    }

    #[inline]
    pub fn destroy_resource<R: Resource, M: Marker>(
        &mut self,
        device: &Device,
        index: ResourceIndex<R>,
    ) -> ResourceResult<()>
    where
        ResourceStorageList: Contains<RawCollection<R>, M>,
    {
        let _ = self
            .storage
            .get_mut()
            .pop(index.index)?
            .inner_mut()
            .destroy(device);
        Ok(())
    }

    #[inline]
    pub fn entry<'a, R: Resource, M: Marker>(
        &'a self,
        index: ResourceIndex<R>,
    ) -> ScopedEntryResult<R>
    where
        ResourceStorageList: Contains<RawCollection<R>, M>,
    {
        let ResourceIndex { index } = index;
        self.storage.get().entry(TypedIndex::<R>::new(index))
    }

    #[inline]
    pub fn entry_mut<'a, R: Resource, M: Marker>(
        &'a mut self,
        index: ResourceIndex<R>,
    ) -> ScopedEntryMutResult<'a, R>
    where
        ResourceStorageList: Contains<RawCollection<R>, M>,
    {
        let ResourceIndex { index } = index;
        self.storage
            .get_mut()
            .entry_mut(TypedIndex::<R>::new(index))
    }

    #[inline]
    fn destroy_resource_storage<R: 'static, M: Marker>(
        &mut self,
        device: &Device,
    ) -> DestroyResult<DropGuard<R>>
    where
        for<'a> R: Destroy<Context<'a> = &'a Device>,
        ResourceStorageList: Contains<GuardCollection<R>, M>,
    {
        self.storage.get_mut().destroy(device)
    }
}

impl Create for ResourceStorage {
    type Config<'a> = ();
    type CreateError = ResourceError;

    fn create<'a, 'b>(_: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
        Ok(ResourceStorage::new())
    }
}

impl Destroy for ResourceStorage {
    type Context<'a> = &'a Device;
    type DestroyError = DropGuardError<Infallible>;

    fn destroy<'a>(&mut self, device: Self::Context<'a>) -> DestroyResult<Self> {
        self.destroy_resource_storage::<BufferRaw, _>(device)?;
        Ok(())
    }
}

use std::{
    any::type_name,
    convert::Infallible,
    error::Error,
    fmt::{Debug, Display, Formatter},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{Create, CreateResult, Destroy, DestroyResult, FromGuard, Valid};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let list = Nil::new().append(3.14).append(42).append("Item");
        let i32_item = list.get::<i32, _>();
        let f32_item = list.get::<f32, _>();
        let str_item = list.get::<&str, _>();
        assert_eq!(*i32_item, 42);
        assert_eq!(*f32_item, 3.14);
        assert_eq!(*str_item, "Item");
    }

    #[test]
    fn test_type_list_len() {
        let list = Nil::new().append(3.14).append(42).append("Item");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_append() {
        let list = Nil::new().append(3.14).append(42).append("Item");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_nil_types_are_empty() {
        let nil = Nil::new();
        assert!(nil.is_empty());
        assert_eq!(nil.len(), 0);
    }
}

pub trait Marker {}

#[derive(Debug, Default, Clone, Copy)]
pub struct Here {}

impl Marker for Here {}

pub struct There<T> {
    _phantom: PhantomData<T>,
}

impl<T> Debug for There<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("There")
            .field("T", &type_name::<T>())
            .finish()
    }
}

impl<T> Default for There<T> {
    #[inline]
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Clone for There<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for There<T> {}

impl<T> Marker for There<T> {}

pub trait Contains<T, M: Marker> {
    fn get(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Marked<T, M: Marker> {
    pub value: T,
    _marker: PhantomData<M>,
}

impl<T, M: Marker> Marked<T, M> {
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }
}

impl<T, M: Marker> Deref for Marked<T, M> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, M: Marker> DerefMut for Marked<T, M> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub struct TypedNil<T> {
    _phantom: PhantomData<T>,
}

impl<T> Debug for TypedNil<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("TypedNil")
            .field("T", &type_name::<T>())
            .finish()
    }
}

impl<T> Clone for TypedNil<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TypedNil<T> {}

impl<T> Default for TypedNil<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> PartialEq for TypedNil<T> {
    #[inline]
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for TypedNil<T> {}

impl<T> TypedNil<T> {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

pub type Nil = TypedNil<()>;

#[derive(Debug, Default, Clone, Copy)]
pub struct Cons<H, T> {
    pub head: H,
    pub tail: T,
}

impl<S, N> Contains<S, Here> for Cons<S, N> {
    #[inline]
    fn get(&self) -> &S {
        &self.head
    }

    #[inline]
    fn get_mut(&mut self) -> &mut S {
        &mut self.head
    }
}

impl<O, S, T: Marker, N: Contains<S, T>> Contains<S, There<T>> for Cons<O, N> {
    #[inline]
    fn get(&self) -> &S {
        self.tail.get()
    }

    #[inline]
    fn get_mut(&mut self) -> &mut S {
        self.tail.get_mut()
    }
}

impl<H, T> Cons<H, T> {
    #[inline]
    pub fn new(head: H, tail: T) -> Self {
        Self { head, tail }
    }

    #[inline]
    pub fn get<S, M: Marker>(&self) -> &S
    where
        Self: Contains<S, M>,
    {
        <Self as Contains<S, M>>::get(self)
    }

    #[inline]
    pub fn get_mut<S, M: Marker>(&mut self) -> &mut S
    where
        Self: Contains<S, M>,
    {
        <Self as Contains<S, M>>::get_mut(self)
    }
}

impl<H, T> Deref for Cons<H, T> {
    type Target = H;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.head
    }
}

impl<H, T> DerefMut for Cons<H, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.head
    }
}

impl<H: PartialEq, T: PartialEq> PartialEq for Cons<H, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.head == other.head && self.tail == other.tail
    }
}

impl<H: Eq, T: Eq> Eq for Cons<H, T> {}

pub trait TypeList: Sized {
    const LEN: usize;
    type Item;
    type Next: TypeList;

    #[inline]
    fn len(&self) -> usize {
        Self::LEN
    }

    #[inline]
    fn is_empty(&self) -> bool {
        Self::LEN == 0
    }

    #[inline]
    fn append<N>(self, item: N) -> Cons<N, Self> {
        Cons::new(item, self)
    }
}

impl<N> TypeList for TypedNil<N> {
    const LEN: usize = 0;
    type Item = N;
    type Next = Self;
}

impl<T, N: TypeList> TypeList for Cons<T, N> {
    const LEN: usize = N::LEN + 1;
    type Item = T;
    type Next = N;
}

#[cfg(test)]
mod test_macro {
    use crate::{list_type, list_value, unpack_list, Cons, Nil, TypedNil};

    trait AssertEqualTypes<A, B> {}

    impl<T> AssertEqualTypes<T, T> for () {}

    #[test]
    fn test_type_list_macro_generates_correct_type() {
        type GeneratedList = list_type![u8, u16, u32];
        type ExpectedList = Cons<u8, Cons<u16, Cons<u32, Nil>>>;

        // Compile-time assertion to check if the types are the same
        let _: &dyn AssertEqualTypes<GeneratedList, ExpectedList> = &();
    }

    #[test]
    fn text_list_macro_generates_correct_value() {
        let list = list_value![8u8, 16u16, 32u32];
        let expected_list = Cons::new(8u8, Cons::new(16u16, Cons::new(32u32, Nil::new())));

        assert_eq!(list, expected_list);
    }

    #[test]
    fn test_unpack_list_macro() {
        let list = list_value![8u8, 16u16, 32u32];
        let unpack_list![value_u8, value_u16, value_u32] = list;

        assert_eq!(value_u8, 8u8);
        assert_eq!(value_u16, 16u16);
        assert_eq!(value_u32, 32u32);
    }
}

#[macro_export]
macro_rules! list_type {
    [] => {
        Nil
    };
    [$head:ty $(, $tail:ty)*] => {
        Cons<$head, list_type![$($tail),*]>
    };
}

#[macro_export]
macro_rules! list_value {
    [] => {
        Nil::new()
    };
    [$head:expr $(, $tail:expr)*] => {
        Cons::new($head, list_value![$($tail),*])
    };
}

#[macro_export]
macro_rules! unpack_list {
    [] => {
        TypedNil { .. }
    };
    [$head:ident $(, $tail:ident)*] => {
        Cons {
            head: $head,
            tail: unpack_list![$($tail),*]
        }
    };
}

impl<T: 'static> FromGuard for TypedNil<T> {
    type Inner = Self;

    #[inline]
    fn into_inner(self) -> Self::Inner {
        self
    }
}

impl<T> From<Valid<TypedNil<T>>> for TypedNil<T> {
    #[inline]
    fn from(_: Valid<TypedNil<T>>) -> Self {
        TypedNil::new()
    }
}

impl<T> Create for TypedNil<T> {
    type Config<'a> = TypedNil<T>;
    type CreateError = Infallible;

    #[inline]
    fn create<'a, 'b>(_: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
        Ok(TypedNil::new())
    }
}

impl<T> Destroy for TypedNil<T> {
    type Context<'a> = &'a ();
    type DestroyError = Infallible;

    #[inline]
    fn destroy<'a>(&mut self, _: Self::Context<'a>) -> DestroyResult<Self> {
        Ok(())
    }
}

impl<H: Create, T> Create for Cons<H, T>
where
    T: Create<CreateError = H::CreateError>,
    for<'a> T: Destroy<Context<'a> = H::Context<'a>>,
    for<'a> H::Context<'a>: Clone + Copy,
{
    type Config<'a> = Cons<H::Config<'a>, T::Config<'a>>;
    type CreateError = H::CreateError;

    #[inline]
    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        let Cons { head, tail } = config;
        let head = H::create(head, context)?;
        let tail = T::create(tail, context)?;
        Ok(Cons::new(head, tail))
    }
}

pub enum ConsDestroyError<H: Destroy, T: Destroy> {
    Head(H::DestroyError),
    Tail(T::DestroyError),
}

impl<H: Destroy, T: Destroy> Debug for ConsDestroyError<H, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Head(arg0) => f.debug_tuple("Head").field(arg0).finish(),
            Self::Tail(arg0) => f.debug_tuple("Tail").field(arg0).finish(),
        }
    }
}

impl<H: Destroy, T: Destroy> Display for ConsDestroyError<H, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Head(arg0) => write!(f, "Head({})", arg0),
            Self::Tail(arg0) => write!(f, "Tail({})", arg0),
        }
    }
}

impl<H: Destroy, T: Destroy> Error for ConsDestroyError<H, T> {}

impl<H: Destroy, T: Destroy> Destroy for Cons<H, T>
where
    for<'a> T: Destroy<Context<'a> = H::Context<'a>>,
    for<'a> H::Context<'a>: Clone + Copy,
{
    type Context<'a> = T::Context<'a>;
    type DestroyError = ConsDestroyError<H, T>;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.head
            .destroy(context)
            .map_err(|err| ConsDestroyError::Head(err))?;
        self.tail
            .destroy(context)
            .map_err(|err| ConsDestroyError::Tail(err))?;
        Ok(())
    }
}

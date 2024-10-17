use std::{any::type_name, fmt::Debug, marker::PhantomData};

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

pub struct Here {}

impl Marker for Here {}

pub struct There<T> {
    _phantom: PhantomData<T>,
}

impl<T> Default for There<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Marker for There<T> {}

pub trait Contains<T, M: Marker> {
    fn get(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
}

pub struct TypedNil<T> {
    _phantom: PhantomData<T>,
}

impl<T> Debug for TypedNil<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl<T> TypedNil<T> {
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
    use crate::{type_list, Cons, Nil};

    trait AssertEqualTypes<A, B> {}

    impl<T> AssertEqualTypes<T, T> for () {}

    #[test]
    fn test_type_list_macro_generates_correct_type() {
        type GeneratedList = type_list![u8, u16, u32];
        type ExpectedList = Cons<u8, Cons<u16, Cons<u32, Nil>>>;

        // Compile-time assertion to check if the types are the same
        let _: &dyn AssertEqualTypes<GeneratedList, ExpectedList> = &();
    }
}

#[macro_export]
macro_rules! type_list {
    [] => {
        Nil
    };
    [$head:ty $(, $tail:ty)*] => {
        Cons<$head, type_list![$($tail),*]>
    };
}

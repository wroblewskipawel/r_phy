use std::{
    any::type_name,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
};

pub trait Marker {}

pub struct Here {}

impl Marker for Here {}

pub struct There<T> {
    _phantom: PhantomData<T>,
}

impl<T> Marker for There<T> {}

pub trait Contains<T, M: Marker> {
    fn get(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Nil {}

#[derive(Clone, Copy)]
pub struct TypedNil<T> {
    _phantom: PhantomData<T>,
}

impl<T> Debug for TypedNil<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("TypedNil<{}>", type_name::<T>()))
    }
}

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

#[derive(Debug, Clone, Copy, Default)]
pub struct Cons<H, T> {
    pub(crate) head: H,
    pub(crate) tail: T,
}

impl<S, N> Contains<S, Here> for Cons<S, N> {
    fn get(&self) -> &S {
        &self.head
    }

    fn get_mut(&mut self) -> &mut S {
        &mut self.head
    }
}

impl<O, S, T: Marker, N: Contains<S, T>> Contains<S, There<T>> for Cons<O, N> {
    fn get(&self) -> &S {
        self.tail.get()
    }

    fn get_mut(&mut self) -> &mut S {
        self.tail.get_mut()
    }
}

pub trait TypeList {
    const LEN: usize;
    type Item;
    type Next: TypeList;
}

impl TypeList for Nil {
    const LEN: usize = 0;
    type Item = ();
    type Next = Nil;
}

impl<T, N: TypeList> TypeList for Cons<T, N> {
    const LEN: usize = 1;
    type Item = T;
    type Next = N;
}

pub trait Transmute<T> {
    type Output;

    fn transmute(self) -> Self::Output;
}

impl<T> Transmute<T> for Nil {
    type Output = Nil;

    fn transmute(self) -> Self::Output {
        self
    }
}

impl<O, T: Transmute<O>, N: Transmute<O>> Transmute<O> for Cons<T, N> {
    type Output = Cons<T::Output, N::Output>;

    fn transmute(self) -> Self::Output {
        Cons {
            head: self.head.transmute(),
            tail: self.tail.transmute(),
        }
    }
}

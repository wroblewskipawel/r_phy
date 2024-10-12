use std::{fmt::Debug, marker::PhantomData};

pub trait Marker: Debug {}

#[derive(Debug)]
pub struct Here {}

impl Marker for Here {}

#[derive(Debug)]
pub struct There<T: Debug> {
    _phantom: PhantomData<T>,
}

impl<T: Debug> Default for There<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: Debug> Marker for There<T> {}

pub trait Contains<T, M: Marker> {
    fn get(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Nil {}

#[derive(Debug)]
pub struct TypedNil<T> {
    _phantom: PhantomData<T>,
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

#[derive(Debug)]
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
    pub fn new(head: H, tail: T) -> Self {
        Self { head, tail }
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
    type Next = Self;
}

impl<N> TypeList for TypedNil<N> {
    const LEN: usize = 0;
    type Item = N;
    type Next = Self;
}

impl<T, N: TypeList> TypeList for Cons<T, N> {
    const LEN: usize = 1;
    type Item = T;
    type Next = N;
}

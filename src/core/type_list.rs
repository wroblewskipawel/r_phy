use std::marker::PhantomData;

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

pub struct Nil {}

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

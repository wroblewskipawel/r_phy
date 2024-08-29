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

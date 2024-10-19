#[cfg(test)]
pub(crate) mod test_types {
    use super::Destroy;

    #[derive(Debug)]
    pub struct C;

    #[derive(Debug)]
    pub struct A;

    impl Destroy for A {
        type Context<'a> = &'a mut C;

        fn destroy<'a>(&mut self, _context: Self::Context<'a>) {}
    }

    #[derive(Debug)]
    pub struct B;

    impl Destroy for B {
        type Context<'a> = ();

        fn destroy<'a>(&mut self, _context: Self::Context<'a>) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_types::{A, B, C};

    #[test]
    fn test_drop_guard_destroyed_before_drop() {
        let mut c = C {};
        let mut a = DropGuard::new(A {});
        a.destroy(&mut c);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_drop_guard_not_destroyed_panic_on_drop_in_debug() {
        let _ = DropGuard::new(A {});
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_drop_guard_not_destroyed_no_panic_on_drop_in_release() {
        let _ = DropGuard::new(A {});
    }

    #[test]
    fn test_drop_finalize_blanket_impl() {
        let mut b = DropGuard::new(B {});
        b.finalize();
    }
}

use std::{
    any::type_name,
    ops::{Deref, DerefMut},
};

pub trait Destroy: Sized {
    type Context<'a>;

    fn destroy<'a>(&mut self, context: Self::Context<'a>);
}

pub trait Finalize: Destroy
where
    Self::Context<'static>: Default,
{
    #[inline]
    fn finalize(&mut self) {
        self.destroy(Self::Context::default());
    }
}

impl<T: Destroy> Finalize for T where T::Context<'static>: Default {}

impl<I: Destroy + 'static, T> Destroy for T
where
    for<'a> I::Context<'a>: Clone + Copy,
    for<'a> &'a mut T: IntoIterator<Item = &'a mut I>,
{
    type Context<'b> = I::Context<'b>;

    #[inline]
    fn destroy<'b>(&mut self, context: Self::Context<'b>) {
        for item in self.into_iter() {
            item.destroy(context);
        }
    }
}

#[derive(Debug)]
pub struct DropGuard<T: Destroy> {
    #[cfg(debug_assertions)]
    inner: Option<T>,
    #[cfg(not(debug_assertions))]
    inner: T,
}

impl<T: Destroy> DropGuard<T> {
    #[inline]
    pub fn new(inner: T) -> Self {
        #[cfg(debug_assertions)]
        let inner = Some(inner);
        Self { inner }
    }
}

impl<T: Destroy> Destroy for DropGuard<T> {
    type Context<'a> = T::Context<'a>;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        #[cfg(debug_assertions)]
        let mut inner_mut = self.inner.take();
        #[cfg(not(debug_assertions))]
        let mut inner_mut = Some(&mut self.inner);
        inner_mut
            .take()
            .as_mut()
            .map(|inner| inner.destroy(context));
    }
}

impl<T: Destroy> Deref for DropGuard<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        #[cfg(debug_assertions)]
        let inner = self.inner.as_ref().unwrap();
        #[cfg(not(debug_assertions))]
        let inner = &self.inner;
        inner
    }
}

impl<T: Destroy> DerefMut for DropGuard<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(debug_assertions)]
        let inner = self.inner.as_mut().unwrap();
        #[cfg(not(debug_assertions))]
        let inner = &mut self.inner;
        inner
    }
}

impl<T: Destroy> Drop for DropGuard<T> {
    #[inline]
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if self.inner.is_some() {
            panic!(
                "DropGuard<{}> inner resource was not destroyed before drop!",
                &type_name::<T>(),
            )
        }
    }
}

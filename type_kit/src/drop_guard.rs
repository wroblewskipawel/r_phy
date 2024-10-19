#[cfg(test)]
pub(crate) mod test_types {
    use super::Destroy;

    #[derive(Debug)]
    pub struct C;

    #[derive(Debug)]
    pub struct A;

    impl Destroy for A {
        type Context = C;

        fn destroy(&mut self, _context: &mut Self::Context) {}
    }

    #[derive(Debug)]
    pub struct B;

    impl Destroy for B {
        type Context = ();

        fn destroy(&mut self, _context: &mut Self::Context) {}
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
    type Context;

    fn destroy(&mut self, context: &mut Self::Context);
}

pub trait Finalize: Destroy
where
    Self::Context: Default,
{
    #[inline]
    fn finalize(&mut self) {
        self.destroy(&mut Self::Context::default());
    }
}

impl<T: Destroy> Finalize for T where T::Context: Default {}

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
    type Context = T::Context;

    #[inline]
    fn destroy(&mut self, context: &mut Self::Context) {
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

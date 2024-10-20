#[cfg(test)]
pub(crate) mod test_types {
    use std::{
        error::Error,
        fmt::{Display, Formatter},
    };

    use super::{Create, CreateResult, Destroy};

    #[derive(Debug)]
    pub struct E;

    impl Display for E {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "E")
        }
    }

    impl Error for E {}

    #[derive(Debug)]
    pub struct C;

    #[derive(Debug)]
    pub struct A(pub u32);

    impl Create for A {
        type Config<'a> = u32;
        type CreateError = E;

        fn create<'a, 'b>(config: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
            Ok(Self(config))
        }
    }

    impl Destroy for A {
        type Context<'a> = &'a C;
        fn destroy<'a>(&mut self, _context: Self::Context<'a>) {}
    }

    #[derive(Debug)]
    pub struct B(pub u32);

    impl Create for B {
        type Config<'a> = u32;
        type CreateError = E;

        fn create<'a, 'b>(config: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
            Ok(Self(config))
        }
    }

    impl Destroy for B {
        type Context<'a> = ();
        fn destroy<'a>(&mut self, _context: Self::Context<'a>) {}
    }

    #[derive(Debug)]
    pub struct Failling;

    impl Create for Failling {
        type Config<'a> = ();
        type CreateError = E;

        fn create<'a, 'b>(_: Self::Config<'a>, _: Self::Context<'b>) -> CreateResult<Self> {
            Err(E)
        }
    }

    impl Destroy for Failling {
        type Context<'a> = ();
        fn destroy<'a>(&mut self, _: Self::Context<'a>) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_types::{Failling, A, B, C, E};

    #[test]
    fn test_drop_guard_destroyed_before_drop() {
        let mut c = C {};
        let mut a = DropGuard::new(A(42));
        assert_eq!(a.0, 42);
        a.destroy(&mut c);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_drop_guard_not_destroyed_panic_on_drop_in_debug() {
        let _ = DropGuard::new(A(42));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_drop_guard_not_destroyed_no_panic_on_drop_in_release() {
        let _ = DropGuard::new(A(42));
    }

    #[test]
    fn test_drop_into_blanket_impl() {
        let mut b: DropGuard<_> = B(42).into();
        assert_eq!(b.0, 42);
        b.finalize();
    }

    #[test]
    fn test_drop_create_destroy_blanket_impl() {
        let c = C {};
        let mut a = DropGuard::<A>::create(42, &c).unwrap();
        assert_eq!(a.0, 42);
        a.destroy(&C);
    }

    #[test]
    fn test_drop_initialize_and_finalize_blanket_impl() {
        let mut b = DropGuard::<B>::initialize(42).unwrap();
        assert_eq!(b.0, 42);
        b.finalize();
    }

    #[test]
    fn test_drop_create_and_destroy_collection() {
        let c = C {};
        let mut b: Vec<DropGuard<A>> = (0..4u32).create(&c).collect::<Result<_, _>>().unwrap();
        for (value, guard) in (0..4u32).zip(&b) {
            assert_eq!(guard.0, value);
        }
        b.iter_mut().destroy(&c);
    }

    #[test]
    fn test_drop_initialize_and_finalize_collection() {
        let mut b: Vec<DropGuard<B>> = (0..4u32).initialize().collect::<Result<_, _>>().unwrap();
        for (value, guard) in (0..4u32).zip(&b) {
            assert_eq!(guard.0, value);
        }
        b.iter_mut().finalize();
    }

    #[test]
    fn test_create_failure_returns_error() {
        let result = Failling::initialize(());
        assert!(matches!(result, Err(E {})));
    }

    #[test]
    fn test_guard_create_failure_returns_inner_type_error() {
        let result = DropGuard::<Failling>::initialize(());
        assert!(matches!(result, Err(E {})));
    }
}

use std::{
    any::type_name,
    error::Error,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

pub type CreateResult<T> = Result<T, <T as Create>::CreateError>;

pub trait Create: Destroy {
    type Config<'a>;
    type CreateError: Error;

    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self>;
}

pub trait Destroy: Sized {
    type Context<'a>;
    fn destroy<'a>(&mut self, context: Self::Context<'a>);
}

pub trait Initialize: Create
where
    Self::Context<'static>: Default,
{
    #[inline]
    fn initialize<'a>(config: Self::Config<'a>) -> CreateResult<Self> {
        Self::create(config, Self::Context::default())
    }
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

impl<T: Create> Initialize for T where T::Context<'static>: Default {}
impl<T: Destroy> Finalize for T where T::Context<'static>: Default {}

pub trait CreateCollection<I: Create>: Sized + IntoIterator
where
    for<'a> I::Context<'a>: Clone + Copy,
    for<'a> Self::Item: Into<I::Config<'a>>,
{
    #[inline]
    fn create<'a>(self, context: I::Context<'a>) -> impl Iterator<Item = CreateResult<I>> {
        self.into_iter()
            .map(move |config| I::create(config.into(), context))
    }
}

pub trait DestroyCollection<I: Destroy>: Sized + IntoIterator
where
    for<'a> I::Context<'a>: Clone + Copy,
    for<'a> Self::Item: DerefMut<Target = I>,
{
    #[inline]
    fn destroy<'a>(self, context: I::Context<'a>) {
        self.into_iter().for_each(|mut item| item.destroy(context));
    }
}

impl<T: Create, I: Sized + IntoIterator> CreateCollection<T> for I
where
    for<'a> T::Context<'a>: Clone + Copy,
    for<'a> I::Item: Into<T::Config<'a>>,
{
}

impl<T: Destroy, I: Sized + IntoIterator> DestroyCollection<T> for I
where
    for<'a> T::Context<'a>: Clone + Copy,
    for<'a> Self::Item: DerefMut<Target = T>,
{
}

pub trait InitializeCollection<I: Initialize>: Sized + IntoIterator
where
    for<'a> I::Context<'a>: Default,
    for<'a> Self::Item: Into<I::Config<'a>>,
{
    #[inline]
    fn initialize<'a>(self) -> impl Iterator<Item = CreateResult<I>> {
        self.into_iter()
            .map(move |config| I::create(config.into(), I::Context::default()))
    }
}

pub trait FinalizeCollection<I: Finalize>: Sized + IntoIterator
where
    for<'a> I::Context<'a>: Default,
    for<'a> Self::Item: DerefMut<Target = I>,
{
    #[inline]
    fn finalize<'a>(self) {
        self.into_iter()
            .for_each(|mut item| item.destroy(I::Context::default()));
    }
}

impl<T: Initialize, I: Sized + IntoIterator> InitializeCollection<T> for I
where
    for<'a> T::Context<'a>: Default,
    for<'a> I::Item: Into<T::Config<'a>>,
{
}

impl<T: Finalize, I: Sized + IntoIterator> FinalizeCollection<T> for I
where
    for<'a> T::Context<'a>: Default,
    for<'a> Self::Item: DerefMut<Target = T>,
{
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

impl<T: Create + Destroy> From<T> for DropGuard<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Create + Destroy> Create for DropGuard<T> {
    type Config<'a> = T::Config<'a>;
    type CreateError = T::CreateError;

    #[inline]
    fn create<'a, 'b>(config: Self::Config<'a>, context: Self::Context<'b>) -> CreateResult<Self> {
        T::create(config, context).map(Self::new)
    }
}

impl<T: Destroy> Destroy for DropGuard<T> {
    type Context<'a> = T::Context<'a>;
    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) {
        #[cfg(debug_assertions)]
        {
            if let Some(mut inner) = self.inner.take() {
                inner.destroy(context);
            }
        }
        #[cfg(not(debug_assertions))]
        {
            self.inner.destroy(context);
        }
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
        // WARNING:
        //
        // While this `DerefMut` implementation allows you to obtain a mutable reference
        // to the inner resource, **do not call `destroy()` directly on the inner resource**.
        //
        // Calling `destroy()` on the inner resource can lead to double-destruction when
        // `DropGuard` attempts to destroy the resource again, potentially causing undefined
        // behavior or resource leaks.
        //
        // Instead, always use `DropGuard::destroy()` to properly destroy the resource.
        // `DropGuard` ensures that the resource is destroyed only once and provides safety
        // checks in debug builds to catch misuse.

        #[cfg(debug_assertions)]
        let inner = self.inner.as_mut().unwrap();
        #[cfg(not(debug_assertions))]
        let inner = &mut self.inner;
        inner
    }
}

impl<T: Destroy> AsRef<T> for DropGuard<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: Destroy> AsMut<T> for DropGuard<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        // WARNING:
        //
        // While this `AsMut` implementation allows you to obtain a mutable reference
        // to the inner resource, **do not call `destroy()` directly on the inner resource**.
        //
        // Calling `destroy()` on the inner resource can lead to double-destruction when
        // `DropGuard` attempts to destroy the resource again, potentially causing undefined
        // behavior or resource leaks.
        //
        // Instead, always use `DropGuard::destroy()` to properly destroy the resource.
        // `DropGuard` ensures that the resource is destroyed only once and provides safety
        // checks in debug builds to catch misuse.

        self
    }
}

impl<T: Destroy> Drop for DropGuard<T> {
    #[inline]
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if self.inner.is_some() {
            panic!(
                "DropGuard<{}> inner resource was not destroyed before drop! \
                 Ensure DropGuard::destroy is called before it's dropped",
                &type_name::<T>(),
            )
        }
    }
}

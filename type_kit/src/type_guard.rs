#[cfg(test)]
pub(crate) mod test_types {
    use super::{FromGuard, Valid};

    #[derive(Debug, Clone, Copy)]
    pub struct A(pub u32);

    impl FromGuard for A {
        type Inner = u32;

        fn into_inner(self) -> Self::Inner {
            self.0
        }
    }

    impl From<Valid<A>> for A {
        fn from(value: Valid<A>) -> Self {
            A(value.into_inner())
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct B(pub u32);

    impl FromGuard for B {
        type Inner = u32;

        fn into_inner(self) -> Self::Inner {
            self.0
        }
    }

    impl From<Valid<B>> for B {
        fn from(value: Valid<B>) -> Self {
            B(value.into_inner())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::{type_name, TypeId};

    use crate::{
        type_guard::test_types::{A, B},
        FromGuard, TypeGuardConversionError,
    };

    #[test]
    fn test_type_guard_valid_conversion() {
        let a = A(42);
        let a_guard = a.into_guard();
        let a = A::try_from_guard(a_guard).unwrap();
        assert_eq!(a.0, 42);

        let b = B(42);
        let b_guard = b.into_guard();
        let b = B::try_from_guard(b_guard).unwrap();
        assert_eq!(b.0, 42);
    }

    #[test]
    fn test_type_guard_invalid_conversion() {
        let a = A(42);
        let a_guard = a.into_guard();
        assert!(B::try_from_guard(a_guard).is_err());

        let b = B(42);
        let b_guard = b.into_guard();
        assert!(A::try_from_guard(b_guard).is_err());
    }

    #[test]
    fn test_type_guard_error_value() {
        let b_type_name = type_name::<B>();
        #[cfg(debug_assertions)]
        let a_type_name = type_name::<A>();
        #[cfg(not(debug_assertions))]
        let a_type_id = TypeId::of::<A>();

        let a = A(42);
        let a_guard = a.into_guard();
        match B::try_from_guard(a_guard) {
            Err(TypeGuardConversionError { to, from }) => {
                assert_eq!(to, b_type_name);
                #[cfg(debug_assertions)]
                assert_eq!(from, a_type_name);
                #[cfg(not(debug_assertions))]
                assert_eq!(from, a_type_id);
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn test_type_guard_error_display() {
        let b_type_name = format!("{:?}", type_name::<B>());
        let a_type_name = format!("{:?}", type_name::<A>());
        let a_type_id = format!("{:?}", TypeId::of::<A>());

        let a = A(42);
        let a_guard = a.into_guard();
        let error = B::try_from_guard(a_guard).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "TypeGuard conversion error: cannot convert from {} to {}",
                if cfg!(debug_assertions) {
                    a_type_name
                } else {
                    a_type_id
                },
                b_type_name
            )
        );
    }
}

use std::{
    any::{type_name, TypeId},
    error::Error,
    fmt::{Display, Formatter},
    marker::PhantomData,
};

pub type Valid<T> = TypeGuardUnlocked<<T as FromGuard>::Inner, T>;
pub type Guard<T> = TypeGuard<<T as FromGuard>::Inner>;
pub type GuardResult<T> = Result<T, TypeGuardConversionError>;

pub trait FromGuard: 'static + From<Valid<Self>> {
    type Inner;

    fn into_inner(self) -> Self::Inner;

    #[inline]
    fn try_from_guard(value: Guard<Self>) -> GuardResult<Self> {
        let value: Conv<Self> = value.try_into()?;
        Ok(value.unwrap())
    }

    #[inline]
    fn into_guard(self) -> Guard<Self> {
        unsafe { TypeGuard::from_inner::<Self>(self.into_inner()) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Conv<T: FromGuard>(T);

impl<T: FromGuard> Conv<T> {
    #[inline]
    pub fn unwrap(self) -> T {
        self.0
    }
}

impl<T: FromGuard> TryFrom<Guard<T>> for Conv<T> {
    type Error = TypeGuardConversionError;

    fn try_from(value: Guard<T>) -> Result<Self, Self::Error> {
        let unlocked: Valid<T> = value.try_into()?;
        Ok(Conv(unlocked.into()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeGuard<T> {
    inner: T,
    type_id: TypeId,
    #[cfg(debug_assertions)]
    type_name: &'static str,
}

impl<I> TypeGuard<I> {
    /// Creates a new `TypeGuard` from the inner value `T::Inner`.
    ///
    /// # Safety
    ///
    /// The `from_inner` method is marked as `unsafe` because there is no way to ensure at compile-time
    /// that the `inner` value passed here was indeed constructed from an instance of the type `T`.
    /// While multiple types can share the same inner type (`T::Inner`), the inner type alone is not
    /// enough to uniquely determine the outer type `T`. This can lead to situations where an inner
    /// value is incorrectly associated with the wrong type `T`, which could cause undefined behavior
    /// when the `TypeGuard` is used.
    #[inline]
    pub unsafe fn from_inner<T: FromGuard<Inner = I>>(inner: T::Inner) -> Self {
        Self {
            inner,
            type_id: TypeId::of::<T>(),
            #[cfg(debug_assertions)]
            type_name: type_name::<T>(),
        }
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[cfg(debug_assertions)]
    #[inline]
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    #[inline]
    pub fn inner(&self) -> &I {
        &self.inner
    }

    #[inline]
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.inner
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeGuardUnlocked<T, U: 'static> {
    inner: T,
    _phantom: PhantomData<U>,
}

impl<T, U: 'static> TypeGuardUnlocked<T, U> {
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U> TryFrom<TypeGuard<T>> for TypeGuardUnlocked<T, U> {
    type Error = TypeGuardConversionError;

    fn try_from(value: TypeGuard<T>) -> Result<Self, Self::Error> {
        let type_id = TypeId::of::<U>();
        if type_id != value.type_id {
            Err(TypeGuardConversionError {
                to: type_name::<U>(),
                #[cfg(debug_assertions)]
                from: value.type_name,
                #[cfg(not(debug_assertions))]
                from: value.type_id,
            })
        } else {
            Ok(TypeGuardUnlocked {
                inner: value.inner,
                _phantom: PhantomData,
            })
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeGuardConversionError {
    to: &'static str,
    #[cfg(debug_assertions)]
    from: &'static str,
    #[cfg(not(debug_assertions))]
    from: TypeId,
}

impl Display for TypeGuardConversionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TypeGuard conversion error: cannot convert from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl Error for TypeGuardConversionError {}

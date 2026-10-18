// `core::convert`.
use crate::marker::Sized;
use crate::result::Result;

pub trait AsRef<T: ?Sized> {
    fn as_ref(&self) -> &T;
}
pub trait AsMut<T: ?Sized> {
    fn as_mut(&mut self) -> &mut T;
}
pub trait From<T>: Sized {
    fn from(value: T) -> Self;
}
pub trait Into<T>: Sized {
    fn into(self) -> T;
}
pub trait TryFrom<T>: Sized {
    type Error;
    fn try_from(value: T) -> Result<Self, Self::Error>;
}
pub trait TryInto<T>: Sized {
    type Error;
    fn try_into(self) -> Result<T, Self::Error>;
}

impl<T, U> Into<U> for T
where
    U: From<T>,
{
    fn into(self) -> U {
        U::from(self)
    }
}
impl<T> From<T> for T {
    fn from(t: T) -> T {
        t
    }
}
impl<T, U> TryInto<U> for T
where
    U: TryFrom<T>,
{
    type Error = <U as TryFrom<T>>::Error;
    fn try_into(self) -> Result<U, <U as TryFrom<T>>::Error> {
        U::try_from(self)
    }
}

// The error type for errors that can never happen.
pub enum Infallible {}

pub fn identity<T>(x: T) -> T {
    x
}

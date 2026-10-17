// `core::future`. Async execution is outside the interpreter's current
// profile; the names resolve.
use crate::marker::Sized;
use crate::pin::Pin;
use crate::task::{Context, Poll};

pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub trait IntoFuture {
    type Output;
    type IntoFuture: Future<Output = Self::Output>;
    fn into_future(self) -> Self::IntoFuture;
}

pub struct Ready<T>(crate::option::Option<T>);

pub fn ready<T>(t: T) -> Ready<T> {
    Ready(crate::option::Option::Some(t))
}

pub struct Pending<T> {
    _data: crate::marker::PhantomData<T>,
}

pub fn pending<T>() -> Pending<T> {
    Pending {
        _data: crate::marker::PhantomData,
    }
}

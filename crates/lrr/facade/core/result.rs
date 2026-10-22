// `core::result`.
use crate::clone::Clone;
use crate::fmt::Debug;
use crate::ops::FnOnce;
use crate::option::Option;

pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    // `<Result<T, E> as Try>::Output`, stated for the interpreter's static
    // `expr?` typing (never called). Not part of the real `Result`.
    #[doc(hidden)]
    pub fn __try_output(self) -> T {
        core::intrinsics::lrr_unsupported("Result::__try_output")
    }
    pub const fn is_ok(&self) -> bool {
        match self {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }
    pub const fn is_err(&self) -> bool {
        match self {
            Result::Ok(_) => false,
            Result::Err(_) => true,
        }
    }
    pub fn is_ok_and(self, f: impl FnOnce(T) -> bool) -> bool {
        match self {
            Result::Err(_) => false,
            Result::Ok(x) => f(x),
        }
    }
    pub fn is_err_and(self, f: impl FnOnce(E) -> bool) -> bool {
        match self {
            Result::Ok(_) => false,
            Result::Err(e) => f(e),
        }
    }
    pub fn ok(self) -> Option<T> {
        match self {
            Result::Ok(x) => Option::Some(x),
            Result::Err(_) => Option::None,
        }
    }
    pub fn err(self) -> Option<E> {
        match self {
            Result::Ok(_) => Option::None,
            Result::Err(x) => Option::Some(x),
        }
    }
    pub const fn as_ref(&self) -> Result<&T, &E> {
        match *self {
            Result::Ok(ref x) => Result::Ok(x),
            Result::Err(ref x) => Result::Err(x),
        }
    }
    pub fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match *self {
            Result::Ok(ref mut x) => Result::Ok(x),
            Result::Err(ref mut x) => Result::Err(x),
        }
    }
    pub fn map<U, F: FnOnce(T) -> U>(self, op: F) -> Result<U, E> {
        match self {
            Result::Ok(t) => Result::Ok(op(t)),
            Result::Err(e) => Result::Err(e),
        }
    }
    pub fn map_or<U, F: FnOnce(T) -> U>(self, default: U, f: F) -> U {
        match self {
            Result::Ok(t) => f(t),
            Result::Err(_) => default,
        }
    }
    pub fn map_err<F, O: FnOnce(E) -> F>(self, op: O) -> Result<T, F> {
        match self {
            Result::Ok(t) => Result::Ok(t),
            Result::Err(e) => Result::Err(op(e)),
        }
    }
    pub fn and<U>(self, res: Result<U, E>) -> Result<U, E> {
        match self {
            Result::Ok(_) => res,
            Result::Err(e) => Result::Err(e),
        }
    }
    pub fn and_then<U, F: FnOnce(T) -> Result<U, E>>(self, op: F) -> Result<U, E> {
        match self {
            Result::Ok(t) => op(t),
            Result::Err(e) => Result::Err(e),
        }
    }
    pub fn or<F>(self, res: Result<T, F>) -> Result<T, F> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(_) => res,
        }
    }
    pub fn or_else<F, O: FnOnce(E) -> Result<T, F>>(self, op: O) -> Result<T, F> {
        match self {
            Result::Ok(t) => Result::Ok(t),
            Result::Err(e) => op(e),
        }
    }
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(t) => t,
            Result::Err(_) => default,
        }
    }
    pub fn unwrap_or_else<F: FnOnce(E) -> T>(self, op: F) -> T {
        match self {
            Result::Ok(t) => t,
            Result::Err(e) => op(e),
        }
    }
    pub fn unwrap_or_default(self) -> T
    where
        T: crate::default::Default,
    {
        match self {
            Result::Ok(x) => x,
            Result::Err(_) => T::default(),
        }
    }
    // Panics with the `Debug` rendering of the error, like the library.
    pub fn unwrap(self) -> T
    where
        E: Debug,
    {
        match self {
            Result::Ok(t) => t,
            Result::Err(_) => {
                core::intrinsics::lrr_unsupported("core::result::Result::unwrap on Err (Debug panic message)")
            }
        }
    }
    pub fn expect(self, msg: &str) -> T
    where
        E: Debug,
    {
        match self {
            Result::Ok(t) => t,
            Result::Err(_) => {
                core::intrinsics::lrr_unsupported("core::result::Result::expect on Err (Debug panic message)")
            }
        }
    }
    pub fn unwrap_err(self) -> E
    where
        T: Debug,
    {
        match self {
            Result::Ok(_) => {
                core::intrinsics::lrr_unsupported("core::result::Result::unwrap_err on Ok (Debug panic message)")
            }
            Result::Err(e) => e,
        }
    }
    pub fn expect_err(self, msg: &str) -> E
    where
        T: Debug,
    {
        match self {
            Result::Ok(_) => {
                core::intrinsics::lrr_unsupported("core::result::Result::expect_err on Ok (Debug panic message)")
            }
            Result::Err(e) => e,
        }
    }
    pub fn iter(&self) -> crate::option::IntoIter<&T> {
        crate::iter::IntoIterator::into_iter(self.as_ref().ok())
    }
}

impl<T, E> Result<Option<T>, E> {
    pub fn transpose(self) -> Option<Result<T, E>> {
        match self {
            Result::Ok(Option::Some(x)) => Option::Some(Result::Ok(x)),
            Result::Ok(Option::None) => Option::None,
            Result::Err(e) => Option::Some(Result::Err(e)),
        }
    }
}

impl<T: Clone, E: Clone> Clone for Result<T, E> {
    fn clone(&self) -> Self {
        match self {
            Result::Ok(x) => Result::Ok(x.clone()),
            Result::Err(x) => Result::Err(x.clone()),
        }
    }
}
impl<T: crate::marker::Copy, E: crate::marker::Copy> crate::marker::Copy for Result<T, E> {}

impl<T: crate::cmp::PartialEq, E: crate::cmp::PartialEq> crate::cmp::PartialEq for Result<T, E> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Result::Ok(a), Result::Ok(b)) => *a == *b,
            (Result::Err(a), Result::Err(b)) => *a == *b,
            _ => false,
        }
    }
}
impl<T: crate::cmp::Eq, E: crate::cmp::Eq> crate::cmp::Eq for Result<T, E> {}

impl<T: crate::fmt::Debug, E: crate::fmt::Debug> crate::fmt::Debug for Result<T, E> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self {
            Result::Ok(value) => f.debug_tuple("Ok").field(value).finish(),
            Result::Err(error) => f.debug_tuple("Err").field(error).finish(),
        }
    }
}

impl<T: crate::cmp::PartialOrd, E: crate::cmp::PartialOrd> crate::cmp::PartialOrd for Result<T, E> {
    fn partial_cmp(&self, other: &Self) -> Option<crate::cmp::Ordering> {
        match (self, other) {
            (Result::Ok(a), Result::Ok(b)) => a.partial_cmp(b),
            (Result::Err(a), Result::Err(b)) => a.partial_cmp(b),
            (Result::Ok(_), Result::Err(_)) => Option::Some(crate::cmp::Ordering::Less),
            (Result::Err(_), Result::Ok(_)) => Option::Some(crate::cmp::Ordering::Greater),
        }
    }
}

// `?` on a `Result`: `Err(e)` short-circuits with the residual
// `Result<Infallible, E>`, and the enclosing function's `FromResidual` impl
// converts the error through `From`.
impl<T, E> crate::ops::Try for Result<T, E> {
    type Output = T;
    type Residual = Result<crate::convert::Infallible, E>;
    fn from_output(output: T) -> Self {
        Result::Ok(output)
    }
    fn branch(self) -> crate::ops::ControlFlow<Result<crate::convert::Infallible, E>, T> {
        match self {
            Result::Ok(value) => crate::ops::ControlFlow::Continue(value),
            Result::Err(error) => crate::ops::ControlFlow::Break(Result::Err(error)),
        }
    }
}
impl<T, E, F: crate::convert::From<E>> crate::ops::FromResidual<Result<crate::convert::Infallible, E>>
    for Result<T, F>
{
    fn from_residual(residual: Result<crate::convert::Infallible, E>) -> Self {
        match residual {
            Result::Err(error) => Result::Err(crate::convert::From::from(error)),
        }
    }
}

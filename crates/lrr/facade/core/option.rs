// `core::option`.
use crate::clone::Clone;
use crate::default::Default;
use crate::ops::{Deref, DerefMut, FnOnce};
use crate::result::Result;

pub enum Option<T> {
    None,
    Some(T),
}

impl<T> Option<T> {
    pub const fn is_some(&self) -> bool {
        match self {
            Option::Some(_) => true,
            Option::None => false,
        }
    }
    pub const fn is_none(&self) -> bool {
        match self {
            Option::Some(_) => false,
            Option::None => true,
        }
    }
    pub fn is_some_and(self, f: impl FnOnce(T) -> bool) -> bool {
        match self {
            Option::None => false,
            Option::Some(x) => f(x),
        }
    }
    pub fn is_none_or(self, f: impl FnOnce(T) -> bool) -> bool {
        match self {
            Option::None => true,
            Option::Some(x) => f(x),
        }
    }
    pub const fn as_ref(&self) -> Option<&T> {
        match *self {
            Option::Some(ref x) => Option::Some(x),
            Option::None => Option::None,
        }
    }
    pub fn as_mut(&mut self) -> Option<&mut T> {
        match *self {
            Option::Some(ref mut x) => Option::Some(x),
            Option::None => Option::None,
        }
    }
    pub fn expect(self, msg: &str) -> T {
        match self {
            Option::Some(val) => val,
            Option::None => core::panicking::panic_display(&msg),
        }
    }
    pub fn unwrap(self) -> T {
        match self {
            Option::Some(val) => val,
            Option::None => core::panicking::panic("called `Option::unwrap()` on a `None` value"),
        }
    }
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => default,
        }
    }
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => f(),
        }
    }
    pub fn unwrap_or_default(self) -> T
    where
        T: Default,
    {
        match self {
            Option::Some(x) => x,
            Option::None => T::default(),
        }
    }
    pub unsafe fn unwrap_unchecked(self) -> T {
        match self {
            Option::Some(val) => val,
            Option::None => core::intrinsics::lrr_unsupported("core::option::Option::unwrap_unchecked on None"),
        }
    }
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U> {
        match self {
            Option::Some(x) => Option::Some(f(x)),
            Option::None => Option::None,
        }
    }
    pub fn inspect<F: FnOnce(&T)>(self, f: F) -> Self {
        if let Option::Some(ref x) = self {
            f(x);
        }
        self
    }
    pub fn map_or<U, F: FnOnce(T) -> U>(self, default: U, f: F) -> U {
        match self {
            Option::Some(t) => f(t),
            Option::None => default,
        }
    }
    pub fn map_or_else<U, D: FnOnce() -> U, F: FnOnce(T) -> U>(self, default: D, f: F) -> U {
        match self {
            Option::Some(t) => f(t),
            Option::None => default(),
        }
    }
    pub fn ok_or<E>(self, err: E) -> Result<T, E> {
        match self {
            Option::Some(v) => Result::Ok(v),
            Option::None => Result::Err(err),
        }
    }
    pub fn ok_or_else<E, F: FnOnce() -> E>(self, err: F) -> Result<T, E> {
        match self {
            Option::Some(v) => Result::Ok(v),
            Option::None => Result::Err(err()),
        }
    }
    pub fn as_deref(&self) -> Option<&T::Target>
    where
        T: Deref,
    {
        match self.as_ref() {
            Option::Some(t) => Option::Some(t.deref()),
            Option::None => Option::None,
        }
    }
    pub fn as_deref_mut(&mut self) -> Option<&mut T::Target>
    where
        T: DerefMut,
    {
        match self.as_mut() {
            Option::Some(t) => Option::Some(t.deref_mut()),
            Option::None => Option::None,
        }
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { inner: self.as_ref() }
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut { inner: self.as_mut() }
    }
    pub fn and<U>(self, optb: Option<U>) -> Option<U> {
        match self {
            Option::Some(_) => optb,
            Option::None => Option::None,
        }
    }
    pub fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Option<U> {
        match self {
            Option::Some(x) => f(x),
            Option::None => Option::None,
        }
    }
    pub fn filter<P: FnOnce(&T) -> bool>(self, predicate: P) -> Self {
        if let Option::Some(x) = self {
            if predicate(&x) {
                return Option::Some(x);
            }
        }
        Option::None
    }
    pub fn or(self, optb: Option<T>) -> Option<T> {
        match self {
            x @ Option::Some(_) => x,
            Option::None => optb,
        }
    }
    pub fn or_else<F: FnOnce() -> Option<T>>(self, f: F) -> Option<T> {
        match self {
            x @ Option::Some(_) => x,
            Option::None => f(),
        }
    }
    pub fn xor(self, optb: Option<T>) -> Option<T> {
        match (self, optb) {
            (a @ Option::Some(_), Option::None) => a,
            (Option::None, b @ Option::Some(_)) => b,
            _ => Option::None,
        }
    }
    pub fn insert(&mut self, value: T) -> &mut T {
        *self = Option::Some(value);
        match self {
            Option::Some(v) => v,
            Option::None => core::panicking::panic("internal error: entered unreachable code"),
        }
    }
    pub fn get_or_insert(&mut self, value: T) -> &mut T {
        if let Option::None = *self {
            *self = Option::Some(value);
        }
        match self {
            Option::Some(v) => v,
            Option::None => core::panicking::panic("internal error: entered unreachable code"),
        }
    }
    pub fn get_or_insert_with<F: FnOnce() -> T>(&mut self, f: F) -> &mut T {
        if let Option::None = *self {
            *self = Option::Some(f());
        }
        match self {
            Option::Some(v) => v,
            Option::None => core::panicking::panic("internal error: entered unreachable code"),
        }
    }
    pub fn take(&mut self) -> Option<T> {
        crate::mem::replace(self, Option::None)
    }
    pub fn take_if<P: FnOnce(&mut T) -> bool>(&mut self, predicate: P) -> Option<T> {
        let take = match self.as_mut() {
            Option::Some(v) => predicate(v),
            Option::None => false,
        };
        if take { self.take() } else { Option::None }
    }
    pub fn replace(&mut self, value: T) -> Option<T> {
        crate::mem::replace(self, Option::Some(value))
    }
    pub fn zip<U>(self, other: Option<U>) -> Option<(T, U)> {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => Option::Some((a, b)),
            _ => Option::None,
        }
    }
}

impl<T> Option<&T> {
    pub fn copied(self) -> Option<T>
    where
        T: crate::marker::Copy,
    {
        match self {
            Option::Some(&v) => Option::Some(v),
            Option::None => Option::None,
        }
    }
    pub fn cloned(self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Option::Some(t) => Option::Some(t.clone()),
            Option::None => Option::None,
        }
    }
}

impl<T> Option<&mut T> {
    pub fn copied(self) -> Option<T>
    where
        T: crate::marker::Copy,
    {
        match self {
            Option::Some(&mut t) => Option::Some(t),
            Option::None => Option::None,
        }
    }
    pub fn cloned(self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Option::Some(t) => Option::Some(t.clone()),
            Option::None => Option::None,
        }
    }
}

impl<T, E> Option<Result<T, E>> {
    pub fn transpose(self) -> Result<Option<T>, E> {
        match self {
            Option::Some(Result::Ok(x)) => Result::Ok(Option::Some(x)),
            Option::Some(Result::Err(e)) => Result::Err(e),
            Option::None => Result::Ok(Option::None),
        }
    }
}

impl<T> Option<Option<T>> {
    pub fn flatten(self) -> Option<T> {
        match self {
            Option::Some(inner) => inner,
            Option::None => Option::None,
        }
    }
}

impl<T: Clone> Clone for Option<T> {
    fn clone(&self) -> Self {
        match self {
            Option::Some(x) => Option::Some(x.clone()),
            Option::None => Option::None,
        }
    }
}
impl<T: crate::marker::Copy> crate::marker::Copy for Option<T> {}

impl<T> Default for Option<T> {
    fn default() -> Option<T> {
        Option::None
    }
}

impl<T: crate::cmp::PartialEq> crate::cmp::PartialEq for Option<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => *a == *b,
            (Option::None, Option::None) => true,
            _ => false,
        }
    }
}
impl<T: crate::cmp::Eq> crate::cmp::Eq for Option<T> {}

impl<T: crate::fmt::Debug> crate::fmt::Debug for Option<T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self {
            Option::Some(value) => f.debug_tuple("Some").field(value).finish(),
            Option::None => f.write_str("None"),
        }
    }
}

impl<T> crate::convert::From<T> for Option<T> {
    fn from(val: T) -> Option<T> {
        Option::Some(val)
    }
}

pub struct Iter<'a, T: 'a> {
    inner: Option<&'a T>,
}
pub struct IterMut<'a, T: 'a> {
    inner: Option<&'a mut T>,
}
pub struct IntoIter<T> {
    inner: Option<T>,
}

impl<'a, T> crate::iter::Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.inner.take()
    }
}
impl<'a, T> crate::iter::Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        self.inner.take()
    }
}
impl<T> crate::iter::Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.inner.take()
    }
}
impl<T> crate::iter::IntoIterator for Option<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { inner: self }
    }
}

impl<T: crate::cmp::PartialOrd> crate::cmp::PartialOrd for Option<T> {
    fn partial_cmp(&self, other: &Self) -> Option<crate::cmp::Ordering> {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => a.partial_cmp(b),
            (Option::None, Option::None) => Option::Some(crate::cmp::Ordering::Equal),
            (Option::None, Option::Some(_)) => Option::Some(crate::cmp::Ordering::Less),
            (Option::Some(_), Option::None) => Option::Some(crate::cmp::Ordering::Greater),
        }
    }
}
impl<T: crate::cmp::Ord> crate::cmp::Ord for Option<T> {
    fn cmp(&self, other: &Self) -> crate::cmp::Ordering {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => a.cmp(b),
            (Option::None, Option::None) => crate::cmp::Ordering::Equal,
            (Option::None, Option::Some(_)) => crate::cmp::Ordering::Less,
            (Option::Some(_), Option::None) => crate::cmp::Ordering::Greater,
        }
    }
}

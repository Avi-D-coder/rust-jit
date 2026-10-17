// `core::default`.
use crate::marker::Sized;

pub trait Default: Sized {
    fn default() -> Self;
}

impl<'a> Default for &'a str {
    fn default() -> &'a str {
        ""
    }
}
impl<A: Default> Default for (A,) {
    fn default() -> (A,) {
        (A::default(),)
    }
}
impl<A: Default, B: Default> Default for (A, B) {
    fn default() -> (A, B) {
        (A::default(), B::default())
    }
}
impl<A: Default, B: Default, C: Default> Default for (A, B, C) {
    fn default() -> (A, B, C) {
        (A::default(), B::default(), C::default())
    }
}

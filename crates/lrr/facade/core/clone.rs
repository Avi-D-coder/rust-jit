// `core::clone`.
use crate::marker::Sized;

pub trait Clone: Sized {
    fn clone(&self) -> Self;
    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }
}

impl<A: Clone> Clone for (A,) {
    fn clone(&self) -> (A,) {
        (self.0.clone(),)
    }
}
impl<A: Clone, B: Clone> Clone for (A, B) {
    fn clone(&self) -> (A, B) {
        (self.0.clone(), self.1.clone())
    }
}
impl<A: Clone, B: Clone, C: Clone> Clone for (A, B, C) {
    fn clone(&self) -> (A, B, C) {
        (self.0.clone(), self.1.clone(), self.2.clone())
    }
}
impl<A: Clone, B: Clone, C: Clone, D: Clone> Clone for (A, B, C, D) {
    fn clone(&self) -> (A, B, C, D) {
        (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone())
    }
}

// Pinned library/core/src/clone.rs:`Share` (unstable `share_trait`): a `Clone`
// whose clone is another alias of the same underlying resource. `share` is
// final in core and defined through `Clone::clone`.
pub trait Share: Clone {
    fn share(&self) -> Self {
        Clone::clone(self)
    }
}
impl<T: ?Sized> Share for &T {}

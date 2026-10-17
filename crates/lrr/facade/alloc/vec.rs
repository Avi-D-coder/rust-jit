// `alloc::vec`. Growable arrays are an interpreter intrinsic.
use core::ops::{Deref, DerefMut};

pub struct Vec<T> {
    buf: *mut T,
    len: usize,
}

pub struct IntoIter<T> {
    buf: Vec<T>,
}

impl<T> Vec<T> {
    pub const fn new() -> Vec<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::new")
    }
    pub fn with_capacity(capacity: usize) -> Vec<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::with_capacity")
    }
    pub fn capacity(&self) -> usize {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::capacity")
    }
    pub fn push(&mut self, value: T) {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::push")
    }
    pub fn pop(&mut self) -> Option<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::pop")
    }
    pub fn insert(&mut self, index: usize, element: T) {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::insert")
    }
    pub fn remove(&mut self, index: usize) -> T {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::remove")
    }
    pub fn truncate(&mut self, len: usize) {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::truncate")
    }
    pub fn clear(&mut self) {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::clear")
    }
    pub fn len(&self) -> usize {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::len")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn as_slice(&self) -> &[T] {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::as_slice")
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::as_mut_slice")
    }
    pub fn extend_from_slice(&mut self, other: &[T])
    where
        T: Clone,
    {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::extend_from_slice")
    }
}

// `vec![elem; n]`.
pub fn from_elem<T: Clone>(elem: T, n: usize) -> Vec<T> {
    core::intrinsics::lrr_unsupported("alloc::vec::from_elem")
}

impl<T> Deref for Vec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Clone> Clone for Vec<T> {
    fn clone(&self) -> Vec<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::clone")
    }
}
impl<T> Default for Vec<T> {
    fn default() -> Vec<T> {
        Vec::new()
    }
}
impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { buf: self }
    }
}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::IntoIter::next")
    }
}
impl<T> core::iter::FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Vec<T> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::from_iter")
    }
}
impl<T> core::iter::Extend<T> for Vec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::extend")
    }
}
impl<T: PartialEq> PartialEq for Vec<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::eq")
    }
}
impl<T> core::ops::Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::index")
    }
}
impl<T> core::ops::IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::index_mut")
    }
}

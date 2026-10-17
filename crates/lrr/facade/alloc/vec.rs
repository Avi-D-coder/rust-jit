// `alloc::vec`. A `Vec<T>` is an interpreter object whose fields are its
// element cells, in index order (see `runtime/cell_vecs.rs`): the declaration
// has no fields of its own, `Vec::new()` is the empty object, and the closed
// intrinsics `lrr_vec_*` add and remove element cells and view them as a
// slice window. Everything else below is ordinary Rust over those operations
// and the slice place projections (`v[i]` and `&v[a..b]` project the element
// cells exactly like an array's).
use core::ops::{Deref, DerefMut};

pub struct Vec<T>;

pub struct IntoIter<T> {
    vec: Vec<T>,
}

impl<T> Vec<T> {
    pub const fn new() -> Vec<T> {
        Vec
    }
    pub fn with_capacity(capacity: usize) -> Vec<T> {
        Vec
    }
    // An empty vector of the same element type (a typed local without a
    // written annotation).
    fn lrr_empty(&self) -> Vec<T> {
        Vec
    }
    pub fn capacity(&self) -> usize {
        self.len()
    }
    pub fn reserve(&mut self, additional: usize) {}
    pub fn reserve_exact(&mut self, additional: usize) {}
    pub fn shrink_to_fit(&mut self) {}
    pub fn len(&self) -> usize {
        core::intrinsics::lrr_vec_len(self)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn push(&mut self, value: T) {
        core::intrinsics::lrr_vec_push(&mut *self, value);
    }
    pub fn pop(&mut self) -> Option<T> {
        let len = self.len();
        if len == 0 {
            None
        } else {
            Some(core::intrinsics::lrr_vec_remove(&mut *self, len - 1))
        }
    }
    pub fn insert(&mut self, index: usize, element: T) {
        if index > self.len() {
            core::panicking::panic("insertion index should be <= len");
        }
        core::intrinsics::lrr_vec_insert(&mut *self, index, element);
    }
    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len() {
            core::panicking::panic("removal index should be < len");
        }
        core::intrinsics::lrr_vec_remove(&mut *self, index)
    }
    pub fn swap_remove(&mut self, index: usize) -> T {
        let len = self.len();
        if index >= len {
            core::panicking::panic("swap_remove index should be < len");
        }
        let last = len - 1;
        if index != last {
            self.as_mut_slice().swap(index, last);
        }
        core::intrinsics::lrr_vec_remove(&mut *self, last)
    }
    // Drops the tail `len..` in index order, like `drop_in_place` of the
    // tail slice.
    pub fn truncate(&mut self, len: usize) {
        while self.len() > len {
            let element = core::intrinsics::lrr_vec_remove(&mut *self, len);
            core::mem::drop(element);
        }
    }
    pub fn clear(&mut self) {
        self.truncate(0);
    }
    pub fn as_slice(&self) -> &[T] {
        core::intrinsics::lrr_vec_as_slice(self)
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        core::intrinsics::lrr_vec_as_mut_slice(&mut *self)
    }
    pub fn extend_from_slice(&mut self, other: &[T])
    where
        T: Clone,
    {
        let mut i: usize = 0;
        let n: usize = other.len();
        while i < n {
            self.push(other[i].clone());
            i += 1;
        }
    }
    pub fn append(&mut self, other: &mut Vec<T>) {
        while other.len() > 0 {
            let element = core::intrinsics::lrr_vec_remove(&mut *other, 0);
            self.push(element);
        }
    }
    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone,
    {
        if new_len < self.len() {
            self.truncate(new_len);
        } else {
            while self.len() < new_len {
                self.push(value.clone());
            }
        }
    }
    pub fn dedup(&mut self)
    where
        T: PartialEq,
    {
        let mut i: usize = 1;
        while i < self.len() {
            if self[i] == self[i - 1] {
                let removed = core::intrinsics::lrr_vec_remove(&mut *self, i);
                core::mem::drop(removed);
            } else {
                i += 1;
            }
        }
    }
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        let mut i: usize = 0;
        while i < self.len() {
            if f(&self[i]) {
                i += 1;
            } else {
                let removed = core::intrinsics::lrr_vec_remove(&mut *self, i);
                core::mem::drop(removed);
            }
        }
    }
    // The drained range is removed at once (as rustc removes it when the
    // `Drain` is dropped) and yielded by value.
    pub fn drain<R>(&mut self, range: R) -> IntoIter<T>
    where
        R: core::ops::LrrIndexRange,
    {
        let len: usize = self.len();
        let start: usize = range.lrr_start();
        let end: usize = range.lrr_end(len);
        if start > end {
            core::panicking::panic("slice index starts after its end");
        }
        if end > len {
            core::panicking::panic("range end index out of range for slice");
        }
        let mut taken = self.lrr_empty();
        let mut count: usize = end - start;
        while count > 0 {
            let element = core::intrinsics::lrr_vec_remove(&mut *self, start);
            taken.push(element);
            count -= 1;
        }
        IntoIter { vec: taken }
    }
    pub fn split_off(&mut self, at: usize) -> Vec<T> {
        let mut tail = self.lrr_empty();
        while self.len() > at {
            let element = core::intrinsics::lrr_vec_remove(&mut *self, at);
            tail.push(element);
        }
        tail
    }
    pub fn into_boxed_slice(self) -> crate::boxed::Box<[T]> {
        core::intrinsics::lrr_unsupported("alloc::vec::Vec::into_boxed_slice")
    }
}

// `vec![elem; n]`.
pub fn from_elem<T: Clone>(elem: T, n: usize) -> Vec<T> {
    let mut v = lrr_empty_like(&elem);
    let mut i: usize = 0;
    while i < n {
        v.push(elem.clone());
        i += 1;
    }
    v
}

fn lrr_empty_like<T>(like: &T) -> Vec<T> {
    Vec
}

// `vec![a, b, ..]`: the array's elements, in order. `into` is an empty
// vector of the element type the (already bound) array fixes.
pub fn lrr_vec_from_array<T, const N: usize>(array: [T; N], mut into: Vec<T>) -> Vec<T> {
    core::intrinsics::lrr_vec_append_array(&mut into, array);
    into
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
        let mut out = self.lrr_empty();
        let mut i: usize = 0;
        let n: usize = self.len();
        while i < n {
            out.push(self[i].clone());
            i += 1;
        }
        out
    }
}
impl<T> Default for Vec<T> {
    fn default() -> Vec<T> {
        Vec::new()
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Vec<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_slice(), f)
    }
}
impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { vec: self }
    }
}
impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> core::slice::Iter<'a, T> {
        self.as_slice().iter()
    }
}
impl<'a, T> IntoIterator for &'a mut Vec<T> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;
    fn into_iter(self) -> core::slice::IterMut<'a, T> {
        self.as_mut_slice().iter_mut()
    }
}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.vec.len() == 0 {
            None
        } else {
            Some(core::intrinsics::lrr_vec_remove(&mut self.vec, 0))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.vec.len(), Some(self.vec.len()))
    }
}
impl<T> core::iter::DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.vec.pop()
    }
}
impl<T> core::iter::ExactSizeIterator for IntoIter<T> {}
impl<T> core::iter::FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Vec<T> {
        lrr_extend_vec(iter, Vec::new())
    }
}
// The vector `out` (of the element type the caller's result context fixes)
// extended with the iterator's items.
fn lrr_extend_vec<T, I: IntoIterator<Item = T>>(iter: I, mut out: Vec<T>) -> Vec<T> {
    let mut it = iter.into_iter();
    while let Some(element) = it.next() {
        out.push(element);
    }
    out
}
impl<T> core::iter::Extend<T> for Vec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Some(element) = it.next() {
            self.push(element);
        }
    }
}
impl<T: PartialEq<U>, U> PartialEq<Vec<U>> for Vec<T> {
    fn eq(&self, other: &Vec<U>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let mut i: usize = 0;
        while i < self.len() {
            if self[i] != other[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}
impl<T: Eq> Eq for Vec<T> {}
impl<T: PartialEq<U>, U, const N: usize> PartialEq<[U; N]> for Vec<T> {
    fn eq(&self, other: &[U; N]) -> bool {
        self.as_slice() == &other[..]
    }
}
impl<'a, T: PartialEq<U>, U, const N: usize> PartialEq<&'a [U; N]> for Vec<T> {
    fn eq(&self, other: &&'a [U; N]) -> bool {
        self.as_slice() == &other[..]
    }
}
impl<T: PartialEq<U>, U> PartialEq<[U]> for Vec<T> {
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice() == other
    }
}
impl<'a, T: PartialEq<U>, U> PartialEq<&'a [U]> for Vec<T> {
    fn eq(&self, other: &&'a [U]) -> bool {
        self.as_slice() == *other
    }
}
impl<'a, T: PartialEq<U>, U> PartialEq<&'a mut [U]> for Vec<T> {
    fn eq(&self, other: &&'a mut [U]) -> bool {
        self.as_slice() == &**other
    }
}
impl<T: PartialEq<U>, U> PartialEq<Vec<U>> for [T] {
    fn eq(&self, other: &Vec<U>) -> bool {
        self == other.as_slice()
    }
}
impl<'a, T: PartialEq<U>, U> PartialEq<Vec<U>> for &'a [T] {
    fn eq(&self, other: &Vec<U>) -> bool {
        *self == other.as_slice()
    }
}
impl<T: PartialOrd> PartialOrd for Vec<T> {
    fn partial_cmp(&self, other: &Vec<T>) -> Option<core::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}
impl<T: Ord> Ord for Vec<T> {
    fn cmp(&self, other: &Vec<T>) -> core::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl<T> core::ops::Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.as_slice()[index]
    }
}
impl<T> core::ops::IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.as_mut_slice()[index]
    }
}
impl<'a, T: Clone> From<&'a [T]> for Vec<T> {
    fn from(slice: &'a [T]) -> Vec<T> {
        lrr_copy_slice(slice, Vec::new())
    }
}
pub(crate) fn lrr_copy_slice<T: Clone>(slice: &[T], mut out: Vec<T>) -> Vec<T> {
    out.extend_from_slice(slice);
    out
}
impl<T> core::borrow::Borrow<[T]> for Vec<T> {
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

// By-value array iteration: the array's elements move into a vector, which
// yields them in order (the elements not yet yielded drop with its iterator).
impl<T, const N: usize> IntoIterator for [T; N] {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        let mut elements = Vec::<T>::new();
        core::intrinsics::lrr_vec_append_array(&mut elements, self);
        elements.into_iter()
    }
}

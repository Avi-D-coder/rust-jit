// `core::slice`. Slice operations are interpreter intrinsics.
pub struct Iter<'a, T: 'a> {
    slice: &'a [T],
    index: usize,
}
pub struct IterMut<'a, T: 'a> {
    slice: &'a mut [T],
    index: usize,
}

impl<'a, T> crate::iter::Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> crate::option::Option<&'a T> {
        core::intrinsics::lrr_unsupported("core::slice::Iter::next")
    }
}
impl<'a, T> crate::iter::Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> crate::option::Option<&'a mut T> {
        core::intrinsics::lrr_unsupported("core::slice::IterMut::next")
    }
}

pub fn from_ref<T>(s: &T) -> &[T] {
    core::intrinsics::lrr_unsupported("core::slice::from_ref")
}

impl<T> [T] {
    pub const fn len(&self) -> usize {
        core::intrinsics::lrr_slice_len(self)
    }
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: crate::cmp::PartialEq<U>, U> crate::cmp::PartialEq<[U]> for [T] {
    fn eq(&self, other: &[U]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let mut i = 0;
        while i < self.len() {
            if self[i] != other[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}
impl<T: crate::cmp::Eq> crate::cmp::Eq for [T] {}

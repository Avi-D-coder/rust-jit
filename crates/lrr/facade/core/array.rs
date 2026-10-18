// `core::array`.
pub struct IntoIter<T, const N: usize> {
    data: [T; N],
    alive: crate::ops::Range<usize>,
}

pub struct TryFromSliceError;

pub fn from_fn<T, const N: usize, F>(cb: F) -> [T; N]
where
    F: crate::ops::FnMut(usize) -> T,
{
    core::intrinsics::lrr_unsupported("core::array::from_fn")
}

impl<T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<[U; N]> for [T; N] {
    // Real core compares the element windows (`SpecArrayEq`); the length
    // is the array's own, so the body needs no const parameter value.
    fn eq(&self, other: &[U; N]) -> bool {
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
impl<T: crate::cmp::Eq, const N: usize> crate::cmp::Eq for [T; N] {}
impl<T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<[U]> for [T; N] {
    fn eq(&self, other: &[U]) -> bool {
        self[..] == *other
    }
}
impl<T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<[U; N]> for [T] {
    fn eq(&self, other: &[U; N]) -> bool {
        *self == other[..]
    }
}
impl<'a, T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<&'a [U]> for [T; N] {
    fn eq(&self, other: &&'a [U]) -> bool {
        self[..] == **other
    }
}
impl<'a, T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<[U; N]> for &'a [T] {
    fn eq(&self, other: &[U; N]) -> bool {
        **self == other[..]
    }
}
impl<'a, T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<&'a mut [U]> for [T; N] {
    fn eq(&self, other: &&'a mut [U]) -> bool {
        self[..] == **other
    }
}
impl<'a, T: crate::cmp::PartialEq<U>, U, const N: usize> crate::cmp::PartialEq<[U; N]> for &'a mut [T] {
    fn eq(&self, other: &[U; N]) -> bool {
        **self == other[..]
    }
}

impl<'a, T, const N: usize> crate::iter::IntoIterator for &'a [T; N] {
    type Item = &'a T;
    type IntoIter = crate::slice::Iter<'a, T>;
    fn into_iter(self) -> crate::slice::Iter<'a, T> {
        self.iter()
    }
}
impl<'a, T, const N: usize> crate::iter::IntoIterator for &'a mut [T; N] {
    type Item = &'a mut T;
    type IntoIter = crate::slice::IterMut<'a, T>;
    fn into_iter(self) -> crate::slice::IterMut<'a, T> {
        self.iter_mut()
    }
}

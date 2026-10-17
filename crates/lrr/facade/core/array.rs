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

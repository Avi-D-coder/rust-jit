// `core::slice`. Slice element access is the interpreter's builtin place
// projection (`s[i]`, `&s[a..b]`); the methods below are ordinary Rust over it.
pub struct Iter<'a, T: 'a> {
    slice: &'a [T],
    index: usize,
    end: usize,
}
pub struct IterMut<'a, T: 'a> {
    slice: &'a mut [T],
    index: usize,
    end: usize,
}

impl<'a, T> crate::iter::Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> crate::option::Option<&'a T> {
        if self.index < self.end {
            let item = &self.slice[self.index];
            self.index += 1;
            crate::option::Option::Some(item)
        } else {
            crate::option::Option::None
        }
    }
    fn size_hint(&self) -> (usize, crate::option::Option<usize>) {
        (self.end - self.index, crate::option::Option::Some(self.end - self.index))
    }
}
impl<'a, T> crate::iter::DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> crate::option::Option<&'a T> {
        if self.index < self.end {
            self.end -= 1;
            crate::option::Option::Some(&self.slice[self.end])
        } else {
            crate::option::Option::None
        }
    }
}
impl<'a, T> crate::iter::ExactSizeIterator for Iter<'a, T> {}
impl<'a, T> crate::clone::Clone for Iter<'a, T> {
    fn clone(&self) -> Iter<'a, T> {
        Iter { slice: self.slice, index: self.index, end: self.end }
    }
}
impl<'a, T> crate::iter::Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> crate::option::Option<&'a mut T> {
        if self.index < self.end {
            let item = &mut self.slice[self.index];
            self.index += 1;
            crate::option::Option::Some(item)
        } else {
            crate::option::Option::None
        }
    }
    fn size_hint(&self) -> (usize, crate::option::Option<usize>) {
        (self.end - self.index, crate::option::Option::Some(self.end - self.index))
    }
}
impl<'a, T> crate::iter::DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> crate::option::Option<&'a mut T> {
        if self.index < self.end {
            self.end -= 1;
            crate::option::Option::Some(&mut self.slice[self.end])
        } else {
            crate::option::Option::None
        }
    }
}
impl<'a, T> crate::iter::ExactSizeIterator for IterMut<'a, T> {}

pub struct Windows<'a, T: 'a> {
    v: &'a [T],
    size: usize,
    pos: usize,
}
impl<'a, T> crate::iter::Iterator for Windows<'a, T> {
    type Item = &'a [T];
    fn next(&mut self) -> crate::option::Option<&'a [T]> {
        if self.pos + self.size <= self.v.len() {
            let window = &self.v[self.pos..self.pos + self.size];
            self.pos += 1;
            crate::option::Option::Some(window)
        } else {
            crate::option::Option::None
        }
    }
}
pub struct Chunks<'a, T: 'a> {
    v: &'a [T],
    size: usize,
    pos: usize,
}
impl<'a, T> crate::iter::Iterator for Chunks<'a, T> {
    type Item = &'a [T];
    fn next(&mut self) -> crate::option::Option<&'a [T]> {
        let len: usize = self.v.len();
        if self.pos < len {
            let mut end: usize = self.pos + self.size;
            if end > len {
                end = len;
            }
            let chunk = &self.v[self.pos..end];
            self.pos = end;
            crate::option::Option::Some(chunk)
        } else {
            crate::option::Option::None
        }
    }
}

impl<'a, T> crate::iter::IntoIterator for &'a [T] {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}
impl<'a, T> crate::iter::IntoIterator for &'a mut [T] {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
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
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { slice: self, index: 0, end: self.len() }
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let end = self.len();
        IterMut { slice: self, index: 0, end: end }
    }
    pub fn first(&self) -> crate::option::Option<&T> {
        if self.len() == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&self[0])
        }
    }
    pub fn last(&self) -> crate::option::Option<&T> {
        let len = self.len();
        if len == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&self[len - 1])
        }
    }
    pub fn first_mut(&mut self) -> crate::option::Option<&mut T> {
        if self.len() == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&mut self[0])
        }
    }
    pub fn last_mut(&mut self) -> crate::option::Option<&mut T> {
        let len = self.len();
        if len == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&mut self[len - 1])
        }
    }
    pub fn get(&self, index: usize) -> crate::option::Option<&T> {
        if index < self.len() {
            crate::option::Option::Some(&self[index])
        } else {
            crate::option::Option::None
        }
    }
    pub fn get_mut(&mut self, index: usize) -> crate::option::Option<&mut T> {
        if index < self.len() {
            crate::option::Option::Some(&mut self[index])
        } else {
            crate::option::Option::None
        }
    }
    pub fn contains(&self, x: &T) -> bool
    where
        T: crate::cmp::PartialEq,
    {
        let mut i: usize = 0;
        while i < self.len() {
            if self[i] == *x {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn swap(&mut self, a: usize, b: usize) {
        let len = self.len();
        if a >= len {
            core::panicking::panic("index out of bounds");
        }
        if b >= len {
            core::panicking::panic("index out of bounds");
        }
        core::intrinsics::lrr_slice_swap(self, a, b);
    }
    pub fn windows(&self, size: usize) -> Windows<'_, T> {
        Windows { v: self, size: size, pos: 0 }
    }
    pub fn chunks(&self, size: usize) -> Chunks<'_, T> {
        Chunks { v: self, size: size, pos: 0 }
    }
    pub fn split_at(&self, mid: usize) -> (&[T], &[T]) {
        let len: usize = self.len();
        (&self[0..mid], &self[mid..len])
    }
    pub fn starts_with(&self, needle: &[T]) -> bool
    where
        T: crate::cmp::PartialEq,
    {
        let n: usize = needle.len();
        if n > self.len() {
            return false;
        }
        let mut i: usize = 0;
        while i < n {
            if self[i] != needle[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    pub fn ends_with(&self, needle: &[T]) -> bool
    where
        T: crate::cmp::PartialEq,
    {
        let n: usize = needle.len();
        let len: usize = self.len();
        if n > len {
            return false;
        }
        let mut i: usize = 0;
        while i < n {
            if self[len - n + i] != needle[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    pub fn reverse(&mut self) {
        let len = self.len();
        let mut i: usize = 0;
        while i < len / 2 {
            core::intrinsics::lrr_slice_swap(&mut *self, i, len - 1 - i);
            i += 1;
        }
    }
}

impl<T: crate::cmp::PartialEq<U>, U> crate::cmp::PartialEq<[U]> for [T] {
    fn eq(&self, other: &[U]) -> bool {
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
impl<T: crate::cmp::Eq> crate::cmp::Eq for [T] {}
impl<T: crate::cmp::PartialOrd> crate::cmp::PartialOrd for [T] {
    fn partial_cmp(&self, other: &[T]) -> Option<crate::cmp::Ordering> {
        let common: usize = if self.len() < other.len() { self.len() } else { other.len() };
        let mut i: usize = 0;
        while i < common {
            match self[i].partial_cmp(&other[i]) {
                Option::Some(crate::cmp::Ordering::Equal) => {}
                different => return different,
            }
            i += 1;
        }
        self.len().partial_cmp(&other.len())
    }
}
impl<T: crate::cmp::Ord> crate::cmp::Ord for [T] {
    fn cmp(&self, other: &[T]) -> crate::cmp::Ordering {
        let common: usize = if self.len() < other.len() { self.len() } else { other.len() };
        let mut i: usize = 0;
        while i < common {
            match self[i].cmp(&other[i]) {
                crate::cmp::Ordering::Equal => {}
                different => return different,
            }
            i += 1;
        }
        self.len().cmp(&other.len())
    }
}

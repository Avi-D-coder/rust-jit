// `alloc::slice`: the slice methods that need an allocation, as ordinary Rust
// over the element place projections and `lrr_slice_swap`.
pub use core::slice::{from_ref, Chunks, Iter, IterMut, Windows};

use crate::string::String;
use crate::vec::Vec;
use core::cmp::Ordering;

// How a stable sort orders two elements.
trait LrrCompare<T> {
    fn lrr_less_eq(&mut self, a: &T, b: &T) -> bool;
}
struct ByOrd;
impl<T: Ord> LrrCompare<T> for ByOrd {
    fn lrr_less_eq(&mut self, a: &T, b: &T) -> bool {
        a.cmp(b).is_le()
    }
}
struct ByFn<F> {
    compare: F,
}
impl<T, F: FnMut(&T, &T) -> Ordering> LrrCompare<T> for ByFn<F> {
    fn lrr_less_eq(&mut self, a: &T, b: &T) -> bool {
        let compare = &mut self.compare;
        compare(&*a, &*b).is_le()
    }
}
struct ByKey<F> {
    key: F,
}
impl<T, K: Ord, F: FnMut(&T) -> K> LrrCompare<T> for ByKey<F> {
    fn lrr_less_eq(&mut self, a: &T, b: &T) -> bool {
        let key = &mut self.key;
        let left = key(&*a);
        let right = key(&*b);
        left.cmp(&right).is_le()
    }
}

fn lrr_indices(n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < n {
        v.push(i);
        i += 1;
    }
    v
}

// A stable merge sort of the element indices (no element moves), then the
// permutation applied by swaps.
fn lrr_merge_sort<T, C: LrrCompare<T>>(s: &mut [T], c: &mut C) {
    let n: usize = s.len();
    if n < 2 {
        return;
    }
    let mut order: Vec<usize> = lrr_indices(n);
    let mut buf: Vec<usize> = lrr_indices(n);
    let mut width: usize = 1;
    while width < n {
        let mut start: usize = 0;
        while start < n {
            let mut mid: usize = start + width;
            if mid > n {
                mid = n;
            }
            let mut end: usize = start + 2 * width;
            if end > n {
                end = n;
            }
            let mut i: usize = start;
            let mut j: usize = mid;
            let mut k: usize = start;
            while i < mid && j < end {
                if c.lrr_less_eq(&s[order[i]], &s[order[j]]) {
                    buf[k] = order[i];
                    i += 1;
                } else {
                    buf[k] = order[j];
                    j += 1;
                }
                k += 1;
            }
            while i < mid {
                buf[k] = order[i];
                i += 1;
                k += 1;
            }
            while j < end {
                buf[k] = order[j];
                j += 1;
                k += 1;
            }
            start = end;
        }
        let mut k: usize = 0;
        while k < n {
            order[k] = buf[k];
            k += 1;
        }
        width *= 2;
    }
    // `dest[original index] = sorted position`; apply by swaps.
    let mut dest: Vec<usize> = lrr_indices(n);
    let mut k: usize = 0;
    while k < n {
        dest[order[k]] = k;
        k += 1;
    }
    let mut i: usize = 0;
    while i < n {
        while dest[i] != i {
            let j: usize = dest[i];
            s.swap(i, j);
            dest.swap(i, j);
        }
        i += 1;
    }
}

impl<T> [T] {
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        crate::vec::lrr_copy_slice(self, Vec::new())
    }
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        lrr_merge_sort(self, &mut ByOrd);
    }
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        lrr_merge_sort(self, &mut ByFn { compare: compare });
    }
    pub fn sort_by_key<K, F>(&mut self, key: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        lrr_merge_sort(self, &mut ByKey { key: key });
    }
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        lrr_merge_sort(self, &mut ByOrd);
    }
    pub fn sort_unstable_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        lrr_merge_sort(self, &mut ByFn { compare: compare });
    }
    pub fn sort_unstable_by_key<K, F>(&mut self, key: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        lrr_merge_sort(self, &mut ByKey { key: key });
    }
}

// `join` and `concat` by the element type's join rule: strings join into a
// `String`, vectors and slices into a `Vec`.
pub trait LrrJoinOut {
    type Output;
}
pub trait LrrJoinElem<Separator>: LrrJoinOut + Sized {
    fn lrr_join(items: &[Self], sep: Separator) -> <Self as LrrJoinOut>::Output;
}
pub trait LrrConcatElem: LrrJoinOut + Sized {
    fn lrr_concat(items: &[Self]) -> <Self as LrrJoinOut>::Output;
}
impl LrrJoinOut for String {
    type Output = String;
}
impl<'a> LrrJoinOut for &'a str {
    type Output = String;
}
impl<T> LrrJoinOut for Vec<T> {
    type Output = Vec<T>;
}
impl<'a, T> LrrJoinOut for &'a [T] {
    type Output = Vec<T>;
}
impl LrrJoinElem<&str> for String {
    fn lrr_join(items: &[String], sep: &str) -> String {
        let mut out = String::new();
        let mut i: usize = 0;
        while i < items.len() {
            if i > 0 {
                out.push_str(sep);
            }
            out.push_str(items[i].as_str());
            i += 1;
        }
        out
    }
}
impl<'a> LrrJoinElem<&str> for &'a str {
    fn lrr_join(items: &[&'a str], sep: &str) -> String {
        let mut out = String::new();
        let mut i: usize = 0;
        while i < items.len() {
            if i > 0 {
                out.push_str(sep);
            }
            out.push_str(items[i]);
            i += 1;
        }
        out
    }
}
impl LrrConcatElem for String {
    fn lrr_concat(items: &[String]) -> String {
        let mut out = String::new();
        let mut i: usize = 0;
        while i < items.len() {
            out.push_str(items[i].as_str());
            i += 1;
        }
        out
    }
}
impl<'a> LrrConcatElem for &'a str {
    fn lrr_concat(items: &[&'a str]) -> String {
        let mut out = String::new();
        let mut i: usize = 0;
        while i < items.len() {
            out.push_str(items[i]);
            i += 1;
        }
        out
    }
}
impl<T: Clone> LrrJoinElem<&T> for Vec<T> {
    fn lrr_join(items: &[Vec<T>], sep: &T) -> Vec<T> {
        let mut out = Vec::<T>::new();
        let mut i: usize = 0;
        while i < items.len() {
            if i > 0 {
                out.push(sep.clone());
            }
            out.extend_from_slice(items[i].as_slice());
            i += 1;
        }
        out
    }
}
impl<T: Clone> LrrJoinElem<&[T]> for Vec<T> {
    fn lrr_join(items: &[Vec<T>], sep: &[T]) -> Vec<T> {
        let mut out = Vec::<T>::new();
        let mut i: usize = 0;
        while i < items.len() {
            if i > 0 {
                out.extend_from_slice(sep);
            }
            out.extend_from_slice(items[i].as_slice());
            i += 1;
        }
        out
    }
}
impl<T: Clone> LrrConcatElem for Vec<T> {
    fn lrr_concat(items: &[Vec<T>]) -> Vec<T> {
        let mut out = Vec::<T>::new();
        let mut i: usize = 0;
        while i < items.len() {
            out.extend_from_slice(items[i].as_slice());
            i += 1;
        }
        out
    }
}
impl<'a, T: Clone> LrrConcatElem for &'a [T] {
    fn lrr_concat(items: &[&'a [T]]) -> Vec<T> {
        let mut out = Vec::<T>::new();
        let mut i: usize = 0;
        while i < items.len() {
            out.extend_from_slice(items[i]);
            i += 1;
        }
        out
    }
}
impl<T> [T] {
    pub fn join<Separator>(&self, sep: Separator) -> <T as LrrJoinOut>::Output
    where
        T: LrrJoinElem<Separator>,
    {
        T::lrr_join(self, sep)
    }
    pub fn concat(&self) -> <T as LrrJoinOut>::Output
    where
        T: LrrConcatElem,
    {
        T::lrr_concat(self)
    }
}

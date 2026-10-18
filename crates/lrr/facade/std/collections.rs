// `std::collections`. The ordered collections are the alloc facade's; the
// hash collections are the same association list in insertion order
// (`LrrMap` with `sorted = false`): iteration order is unspecified, and
// keys are compared with `PartialEq`.
pub use alloc_crate::collections::{
    binary_heap, btree_map, btree_set, linked_list, vec_deque, BTreeMap, BTreeSet, BinaryHeap,
    LinkedList, VecDeque,
};
use alloc_crate::collections::{
    LrrIntoKeys, LrrIter, LrrIterMut, LrrKeyEq, LrrKeyOrd, LrrKeys, LrrMap,
};
use alloc_crate::vec::Vec;

pub struct HashMap<K, V> {
    map: LrrMap<K, V>,
}
pub struct HashSet<T> {
    map: LrrMap<T, ()>,
}

impl<K, V> HashMap<K, V> {
    pub fn new() -> HashMap<K, V> {
        HashMap {
            map: LrrMap::new(false),
        }
    }
    pub fn with_capacity(capacity: usize) -> HashMap<K, V> {
        HashMap {
            map: LrrMap::new(false),
        }
    }
}
impl<K, V> HashMap<K, V> {
    pub fn capacity(&self) -> usize {
        self.map.len()
    }
    pub fn reserve(&mut self, additional: usize) {}
    pub fn shrink_to_fit(&mut self) {}
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V> {
        self.map.get(k)
    }
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V> {
        self.map.get_mut(k)
    }
    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)> {
        self.map.get_key_value(k)
    }
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool {
        self.map.contains_key(k)
    }
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V> {
        self.map.remove(k)
    }
    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)> {
        self.map.remove_entry(k)
    }
    pub fn entry(&mut self, key: K) -> hash_map::Entry<'_, K, V> {
        self.map.entry(key)
    }
    pub fn iter(&self) -> hash_map::Iter<'_, K, V> {
        self.map.iter()
    }
    pub fn iter_mut(&mut self) -> hash_map::IterMut<'_, K, V> {
        self.map.iter_mut()
    }
    pub fn keys(&self) -> hash_map::Keys<'_, K, V> {
        self.map.keys()
    }
    pub fn values(&self) -> hash_map::Values<'_, K, V> {
        self.map.values()
    }
    pub fn values_mut(&mut self) -> hash_map::ValuesMut<'_, K, V> {
        self.map.values_mut()
    }
    pub fn into_keys(self) -> hash_map::IntoKeys<K, V> {
        self.map.into_keys()
    }
    pub fn into_values(self) -> hash_map::IntoValues<K, V> {
        self.map.into_values()
    }
    pub fn retain<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F) {
        self.map.retain(f)
    }
    pub fn drain(&mut self) -> alloc_crate::vec::IntoIter<(K, V)> {
        let taken = core::mem::replace(&mut self.map, LrrMap::new(false));
        taken.lrr_into_entries().into_iter()
    }
}
impl<K: Clone, V: Clone> Clone for HashMap<K, V> {
    fn clone(&self) -> HashMap<K, V> {
        HashMap {
            map: self.map.clone(),
        }
    }
}
impl<K, V, Q: ?Sized> core::ops::Index<&Q> for HashMap<K, V> {
    type Output = V;
    fn index(&self, key: &Q) -> &V {
        match self.get(key) {
            Option::Some(value) => value,
            Option::None => panic!("no entry found for key"),
        }
    }
}
impl<K, V> Default for HashMap<K, V> {
    fn default() -> HashMap<K, V> {
        HashMap {
            map: LrrMap::new(false),
        }
    }
}
impl<K: Eq, V: PartialEq> PartialEq for HashMap<K, V> {
    fn eq(&self, other: &HashMap<K, V>) -> bool {
        self.map == other.map
    }
}
impl<K: Eq, V: Eq> Eq for HashMap<K, V> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for HashMap<K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut map = f.debug_map();
        for (key, value) in self.map.iter() {
            map.entry(key, value);
        }
        map.finish()
    }
}
impl<K, V> IntoIterator for HashMap<K, V> {
    type Item = (K, V);
    type IntoIter = alloc_crate::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> alloc_crate::vec::IntoIter<(K, V)> {
        self.map.lrr_into_entries().into_iter()
    }
}
impl<'a, K, V> IntoIterator for &'a HashMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = LrrIter<'a, K, V>;
    fn into_iter(self) -> LrrIter<'a, K, V> {
        self.map.iter()
    }
}
impl<'a, K, V> IntoIterator for &'a mut HashMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = LrrIterMut<'a, K, V>;
    fn into_iter(self) -> LrrIterMut<'a, K, V> {
        self.map.iter_mut()
    }
}
impl<K: Eq, V> core::iter::FromIterator<(K, V)> for HashMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> HashMap<K, V> {
        let mut map = HashMap::<K, V>::new();
        let mut it = iter.into_iter();
        while let Option::Some(pair) = it.next() {
            map.insert(pair.0, pair.1);
        }
        map
    }
}
impl<K: Eq, V> core::iter::Extend<(K, V)> for HashMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Option::Some(pair) = it.next() {
            self.insert(pair.0, pair.1);
        }
    }
}
impl<K: Eq, V, const N: usize> From<[(K, V); N]> for HashMap<K, V> {
    fn from(pairs: [(K, V); N]) -> HashMap<K, V> {
        let mut map = HashMap::<K, V>::new();
        let mut it = pairs.into_iter();
        while let Option::Some(pair) = it.next() {
            map.insert(pair.0, pair.1);
        }
        map
    }
}

impl<T> HashSet<T> {
    pub fn new() -> HashSet<T> {
        HashSet {
            map: LrrMap::new(false),
        }
    }
    pub fn with_capacity(capacity: usize) -> HashSet<T> {
        HashSet {
            map: LrrMap::new(false),
        }
    }
}
impl<T> HashSet<T> {
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    pub fn reserve(&mut self, additional: usize) {}
    pub fn insert(&mut self, value: T) -> bool {
        match self.map.insert(value, ()) {
            Option::Some(_) => false,
            Option::None => true,
        }
    }
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool {
        self.map.contains_key(value)
    }
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool {
        match self.map.remove(value) {
            Option::Some(_) => true,
            Option::None => false,
        }
    }
    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T> {
        match self.map.remove_entry(value) {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T> {
        match self.map.get_key_value(value) {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn iter(&self) -> LrrKeys<'_, T, ()> {
        self.map.keys()
    }
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, mut f: F) {
        self.map.retain(|key, _| f(key))
    }
    pub fn drain(&mut self) -> LrrIntoKeys<T, ()> {
        let taken = core::mem::replace(&mut self.map, LrrMap::new(false));
        taken.into_keys()
    }
    pub fn is_subset(&self, other: &HashSet<T>) -> bool {
        for item in self.iter() {
            if !other.contains(item) {
                return false;
            }
        }
        true
    }
    pub fn is_superset(&self, other: &HashSet<T>) -> bool {
        other.is_subset(self)
    }
    pub fn is_disjoint(&self, other: &HashSet<T>) -> bool {
        for item in self.iter() {
            if other.contains(item) {
                return false;
            }
        }
        true
    }
    pub fn union<'a>(&'a self, other: &'a HashSet<T>) -> alloc_crate::vec::IntoIter<&'a T> {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            out.push(item);
        }
        for item in other.iter() {
            if !self.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
    pub fn intersection<'a>(&'a self, other: &'a HashSet<T>) -> alloc_crate::vec::IntoIter<&'a T> {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            if other.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
    pub fn difference<'a>(&'a self, other: &'a HashSet<T>) -> alloc_crate::vec::IntoIter<&'a T> {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            if !other.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
    pub fn symmetric_difference<'a>(&'a self, other: &'a HashSet<T>) -> alloc_crate::vec::IntoIter<&'a T> {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            if !other.contains(item) {
                out.push(item);
            }
        }
        for item in other.iter() {
            if !self.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
}
fn lrr_ref_vec<'a, T>() -> Vec<&'a T> {
    Vec::new()
}
impl<T: Clone> Clone for HashSet<T> {
    fn clone(&self) -> HashSet<T> {
        HashSet {
            map: self.map.clone(),
        }
    }
}
impl<T> Default for HashSet<T> {
    fn default() -> HashSet<T> {
        HashSet {
            map: LrrMap::new(false),
        }
    }
}
impl<T: Eq> PartialEq for HashSet<T> {
    fn eq(&self, other: &HashSet<T>) -> bool {
        self.map == other.map
    }
}
impl<T: Eq> Eq for HashSet<T> {}
impl<T: core::fmt::Debug> core::fmt::Debug for HashSet<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut set = f.debug_set();
        for item in self.map.keys() {
            set.entry(item);
        }
        set.finish()
    }
}
impl<T> IntoIterator for HashSet<T> {
    type Item = T;
    type IntoIter = LrrIntoKeys<T, ()>;
    fn into_iter(self) -> LrrIntoKeys<T, ()> {
        self.map.into_keys()
    }
}
impl<'a, T> IntoIterator for &'a HashSet<T> {
    type Item = &'a T;
    type IntoIter = LrrKeys<'a, T, ()>;
    fn into_iter(self) -> LrrKeys<'a, T, ()> {
        self.map.keys()
    }
}
impl<T: Eq> core::iter::FromIterator<T> for HashSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> HashSet<T> {
        let mut set = HashSet::<T>::new();
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            set.insert(item);
        }
        set
    }
}
impl<T: Eq> core::iter::Extend<T> for HashSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            self.insert(item);
        }
    }
}
impl<T: Eq, const N: usize> From<[T; N]> for HashSet<T> {
    fn from(items: [T; N]) -> HashSet<T> {
        let mut set = HashSet::<T>::new();
        let mut it = items.into_iter();
        while let Option::Some(item) = it.next() {
            set.insert(item);
        }
        set
    }
}

pub mod hash_map {
    pub use super::HashMap;
    pub use alloc_crate::collections::LrrEntry as Entry;
    pub use alloc_crate::collections::LrrIntoKeys as IntoKeys;
    pub use alloc_crate::collections::LrrIntoValues as IntoValues;
    pub use alloc_crate::collections::LrrIter as Iter;
    pub use alloc_crate::collections::LrrIterMut as IterMut;
    pub use alloc_crate::collections::LrrKeys as Keys;
    pub use alloc_crate::collections::LrrOccupiedEntry as OccupiedEntry;
    pub use alloc_crate::collections::LrrValues as Values;
    pub use alloc_crate::collections::LrrValuesMut as ValuesMut;
    pub use alloc_crate::collections::LrrVacantEntry as VacantEntry;
    pub struct RandomState {
        k0: u64,
        k1: u64,
    }
    impl RandomState {
        pub fn new() -> RandomState {
            RandomState { k0: 0, k1: 0 }
        }
    }
    impl Clone for RandomState {
        fn clone(&self) -> RandomState {
            RandomState { k0: self.k0, k1: self.k1 }
        }
    }
    impl Default for RandomState {
        fn default() -> RandomState {
            RandomState::new()
        }
    }
    impl core::hash::BuildHasher for RandomState {
        type Hasher = DefaultHasher;
        fn build_hasher(&self) -> DefaultHasher {
            DefaultHasher::new()
        }
    }
    /// SipHash-1-3 with zero keys, as std's `DefaultHasher::new()`.
    pub struct DefaultHasher {
        v0: u64,
        v1: u64,
        v2: u64,
        v3: u64,
        length: usize,
        tail: u64,
        ntail: usize,
    }
    impl DefaultHasher {
        pub fn new() -> DefaultHasher {
            DefaultHasher {
                v0: 0x736f6d6570736575,
                v1: 0x646f72616e646f6d,
                v2: 0x6c7967656e657261,
                v3: 0x7465646279746573,
                length: 0,
                tail: 0,
                ntail: 0,
            }
        }
    }
    impl Clone for DefaultHasher {
        fn clone(&self) -> DefaultHasher {
            DefaultHasher {
                v0: self.v0,
                v1: self.v1,
                v2: self.v2,
                v3: self.v3,
                length: self.length,
                tail: self.tail,
                ntail: self.ntail,
            }
        }
    }
    impl Default for DefaultHasher {
        fn default() -> DefaultHasher {
            DefaultHasher::new()
        }
    }
    impl core::fmt::Debug for DefaultHasher {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("DefaultHasher(SipHasher13 { hasher: Hasher { .. } })")
        }
    }
    fn sip_round(v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64) {
        *v0 = v0.wrapping_add(*v1);
        *v2 = v2.wrapping_add(*v3);
        *v1 = v1.rotate_left(13);
        *v1 ^= *v0;
        *v3 = v3.rotate_left(16);
        *v3 ^= *v2;
        *v0 = v0.rotate_left(32);
        *v2 = v2.wrapping_add(*v1);
        *v0 = v0.wrapping_add(*v3);
        *v1 = v1.rotate_left(17);
        *v1 ^= *v2;
        *v3 = v3.rotate_left(21);
        *v3 ^= *v0;
        *v2 = v2.rotate_left(32);
    }
    /// The little-endian value of `len` (< 8) bytes of `buf` from `start`.
    fn load_le(buf: &[u8], start: usize, len: usize) -> u64 {
        let mut out: u64 = 0;
        let mut i = 0;
        while i < len {
            out |= (buf[start + i] as u64) << (8 * i);
            i += 1;
        }
        out
    }
    impl core::hash::Hasher for DefaultHasher {
        fn write(&mut self, msg: &[u8]) {
            let length = msg.len();
            self.length += length;
            let mut needed = 0;
            if self.ntail != 0 {
                needed = 8 - self.ntail;
                let take = if length < needed { length } else { needed };
                self.tail |= load_le(msg, 0, take) << (8 * self.ntail);
                if length < needed {
                    self.ntail += length;
                    return;
                }
                self.v3 ^= self.tail;
                sip_round(&mut self.v0, &mut self.v1, &mut self.v2, &mut self.v3);
                self.v0 ^= self.tail;
                self.ntail = 0;
            }
            let len = length - needed;
            let left = len & 7;
            let mut i = needed;
            while i < length - left {
                let mi = load_le(msg, i, 7) | ((msg[i + 7] as u64) << 56);
                self.v3 ^= mi;
                sip_round(&mut self.v0, &mut self.v1, &mut self.v2, &mut self.v3);
                self.v0 ^= mi;
                i += 8;
            }
            self.tail = load_le(msg, i, left);
            self.ntail = left;
        }
        fn finish(&self) -> u64 {
            let mut v0 = self.v0;
            let mut v1 = self.v1;
            let mut v2 = self.v2;
            let mut v3 = self.v3;
            let b: u64 = (((self.length as u64) & 0xff) << 56) | self.tail;
            v3 ^= b;
            sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
            v0 ^= b;
            v2 ^= 0xff;
            sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
            sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
            sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
            v0 ^ v1 ^ v2 ^ v3
        }
    }
}
pub mod hash_set {
    pub use super::HashSet;
    pub use super::hash_map::RandomState;
}

// `alloc::collections`. Maps and sets are an association list: a `Vec` of
// `(key, value)` pairs, sorted by key (binary search, the `BTreeMap` order)
// or in insertion order (linear search, `HashMap`, whose iteration order is
// unspecified), implemented once as `LrrMap` and wrapped by the std names.
use crate::string::String;
use crate::vec::Vec;
use core::cmp::Ordering;

// How a stored key compares to a borrowed form of itself (`String` to `str`),
// the role of `Borrow<Q> + Ord`/`Eq` in the real signatures.
pub trait LrrKeyOrd<Q: ?Sized> {
    fn lrr_key_cmp(&self, other: &Q) -> Ordering;
}
pub trait LrrKeyEq<Q: ?Sized> {
    fn lrr_key_eq(&self, other: &Q) -> bool;
}
impl<K: Ord> LrrKeyOrd<K> for K {
    fn lrr_key_cmp(&self, other: &K) -> Ordering {
        self.cmp(other)
    }
}
impl<K: PartialEq> LrrKeyEq<K> for K {
    fn lrr_key_eq(&self, other: &K) -> bool {
        *self == *other
    }
}
impl LrrKeyOrd<String> for String {
    fn lrr_key_cmp(&self, other: &String) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl LrrKeyOrd<str> for String {
    fn lrr_key_cmp(&self, other: &str) -> Ordering {
        self.as_str().cmp(other)
    }
}
impl LrrKeyEq<String> for String {
    fn lrr_key_eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}
impl LrrKeyEq<str> for String {
    fn lrr_key_eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl<'a> LrrKeyOrd<&'a str> for &'a str {
    fn lrr_key_cmp(&self, other: &&'a str) -> Ordering {
        (*self).cmp(*other)
    }
}
impl<'a> LrrKeyOrd<str> for &'a str {
    fn lrr_key_cmp(&self, other: &str) -> Ordering {
        (*self).cmp(other)
    }
}
impl<'a> LrrKeyEq<&'a str> for &'a str {
    fn lrr_key_eq(&self, other: &&'a str) -> bool {
        *self == *other
    }
}
impl<'a> LrrKeyEq<str> for &'a str {
    fn lrr_key_eq(&self, other: &str) -> bool {
        *self == other
    }
}
impl<T: Ord> LrrKeyOrd<Vec<T>> for Vec<T> {
    fn lrr_key_cmp(&self, other: &Vec<T>) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl<T: Ord> LrrKeyOrd<[T]> for Vec<T> {
    fn lrr_key_cmp(&self, other: &[T]) -> Ordering {
        self.as_slice().cmp(other)
    }
}
impl<T: PartialEq> LrrKeyEq<Vec<T>> for Vec<T> {
    fn lrr_key_eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<T: PartialEq> LrrKeyEq<[T]> for Vec<T> {
    fn lrr_key_eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

// An empty vector of references (a typed local without a written annotation).
fn lrr_ref_vec<'a, T>() -> Vec<&'a T> {
    Vec::new()
}

pub struct LrrMap<K, V> {
    entries: Vec<(K, V)>,
    sorted: bool,
}

impl<K, V> LrrMap<K, V> {
    pub fn new(sorted: bool) -> LrrMap<K, V> {
        LrrMap {
            entries: Vec::new(),
            sorted: sorted,
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }
    pub fn clear(&mut self) {
        while self.entries.len() > 0 {
            let dropped = self.entries.pop();
            core::mem::drop(dropped);
        }
    }
    // `Ok(index)` of the entry whose key equals `key`, else `Err(index)` where
    // it would be inserted.
    pub fn locate<Q: ?Sized>(&self, key: &Q) -> Result<usize, usize>
    {
        let len: usize = self.entries.len();
        if self.sorted {
            let mut low: usize = 0;
            let mut high: usize = len;
            while low < high {
                let mid: usize = low + (high - low) / 2;
                match self.entries[mid].0.lrr_key_cmp(key) {
                    Ordering::Less => low = mid + 1,
                    Ordering::Greater => high = mid,
                    Ordering::Equal => return Result::Ok(mid),
                }
            }
            Result::Err(low)
        } else {
            let mut i: usize = 0;
            while i < len {
                if self.entries[i].0.lrr_key_eq(key) {
                    return Result::Ok(i);
                }
                i += 1;
            }
            Result::Err(len)
        }
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    {
        match self.locate(&key) {
            Result::Ok(index) => Option::Some(core::mem::replace(&mut self.entries[index].1, value)),
            Result::Err(index) => {
                self.entries.insert(index, (key, value));
                Option::None
            }
        }
    }
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    {
        match self.locate(key) {
            Result::Ok(index) => Option::Some(&self.entries[index].1),
            Result::Err(_) => Option::None,
        }
    }
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    {
        match self.locate(key) {
            Result::Ok(index) => Option::Some(&mut self.entries[index].1),
            Result::Err(_) => Option::None,
        }
    }
    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    {
        match self.locate(key) {
            Result::Ok(index) => {
                let entry = &self.entries[index];
                Option::Some((&entry.0, &entry.1))
            }
            Result::Err(_) => Option::None,
        }
    }
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    {
        match self.locate(key) {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    {
        match self.locate(key) {
            Result::Ok(index) => {
                let removed = self.entries.remove(index);
                Option::Some(removed.1)
            }
            Result::Err(_) => Option::None,
        }
    }
    pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    {
        match self.locate(key) {
            Result::Ok(index) => Option::Some(self.entries.remove(index)),
            Result::Err(_) => Option::None,
        }
    }
    pub fn entry(&mut self, key: K) -> LrrEntry<'_, K, V>
    {
        match self.locate(&key) {
            Result::Ok(index) => LrrEntry::Occupied(LrrOccupiedEntry {
                entries: &mut self.entries,
                index: index,
            }),
            Result::Err(index) => LrrEntry::Vacant(LrrVacantEntry {
                entries: &mut self.entries,
                index: index,
                key: key,
            }),
        }
    }
    pub fn iter(&self) -> LrrIter<'_, K, V> {
        LrrIter {
            inner: self.entries.iter(),
        }
    }
    pub fn iter_mut(&mut self) -> LrrIterMut<'_, K, V> {
        LrrIterMut {
            inner: self.entries.iter_mut(),
        }
    }
    pub fn keys(&self) -> LrrKeys<'_, K, V> {
        LrrKeys {
            inner: self.entries.iter(),
        }
    }
    pub fn values(&self) -> LrrValues<'_, K, V> {
        LrrValues {
            inner: self.entries.iter(),
        }
    }
    pub fn values_mut(&mut self) -> LrrValuesMut<'_, K, V> {
        LrrValuesMut {
            inner: self.entries.iter_mut(),
        }
    }
    pub fn into_keys(self) -> LrrIntoKeys<K, V> {
        LrrIntoKeys {
            inner: self.entries.into_iter(),
        }
    }
    pub fn into_values(self) -> LrrIntoValues<K, V> {
        LrrIntoValues {
            inner: self.entries.into_iter(),
        }
    }
    pub fn retain<F: FnMut(&K, &mut V) -> bool>(&mut self, mut f: F) {
        let mut i: usize = 0;
        while i < self.entries.len() {
            let keep = {
                let entry = &mut self.entries[i];
                f(&entry.0, &mut entry.1)
            };
            if keep {
                i += 1;
            } else {
                let removed = self.entries.remove(i);
                core::mem::drop(removed);
            }
        }
    }
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        if self.entries.len() == 0 {
            Option::None
        } else {
            let entry = &self.entries[0];
            Option::Some((&entry.0, &entry.1))
        }
    }
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        let len: usize = self.entries.len();
        if len == 0 {
            Option::None
        } else {
            let entry = &self.entries[len - 1];
            Option::Some((&entry.0, &entry.1))
        }
    }
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        if self.entries.len() == 0 {
            Option::None
        } else {
            Option::Some(self.entries.remove(0))
        }
    }
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        self.entries.pop()
    }
    pub fn lrr_into_entries(self) -> Vec<(K, V)> {
        self.entries
    }
    pub fn lrr_entries(&self) -> &Vec<(K, V)> {
        &self.entries
    }
}
impl<K: Clone, V: Clone> Clone for LrrMap<K, V> {
    fn clone(&self) -> LrrMap<K, V> {
        LrrMap {
            entries: self.entries.clone(),
            sorted: self.sorted,
        }
    }
}
impl<K, V: PartialEq> PartialEq for LrrMap<K, V> {
    fn eq(&self, other: &LrrMap<K, V>) -> bool {
        if self.entries.len() != other.entries.len() {
            return false;
        }
        let mut i: usize = 0;
        while i < self.entries.len() {
            match other.get(&self.entries[i].0) {
                Option::Some(value) => {
                    if self.entries[i].1 != *value {
                        return false;
                    }
                }
                Option::None => return false,
            }
            i += 1;
        }
        true
    }
}

pub enum LrrEntry<'a, K: 'a, V: 'a> {
    Occupied(LrrOccupiedEntry<'a, K, V>),
    Vacant(LrrVacantEntry<'a, K, V>),
}
pub struct LrrOccupiedEntry<'a, K: 'a, V: 'a> {
    entries: &'a mut Vec<(K, V)>,
    index: usize,
}
pub struct LrrVacantEntry<'a, K: 'a, V: 'a> {
    entries: &'a mut Vec<(K, V)>,
    index: usize,
    key: K,
}
impl<'a, K, V> LrrEntry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            LrrEntry::Occupied(entry) => entry.into_mut(),
            LrrEntry::Vacant(entry) => entry.insert(default),
        }
    }
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            LrrEntry::Occupied(entry) => entry.into_mut(),
            LrrEntry::Vacant(entry) => entry.insert(default()),
        }
    }
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        match self {
            LrrEntry::Occupied(entry) => entry.into_mut(),
            LrrEntry::Vacant(entry) => entry.insert(V::default()),
        }
    }
    pub fn and_modify<F: FnOnce(&mut V)>(self, f: F) -> LrrEntry<'a, K, V> {
        match self {
            LrrEntry::Occupied(mut entry) => {
                f(entry.get_mut());
                LrrEntry::Occupied(entry)
            }
            LrrEntry::Vacant(entry) => LrrEntry::Vacant(entry),
        }
    }
    pub fn key(&self) -> &K {
        match self {
            LrrEntry::Occupied(entry) => entry.key(),
            LrrEntry::Vacant(entry) => entry.key(),
        }
    }
}
impl<'a, K, V> LrrOccupiedEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.entries[self.index].0
    }
    pub fn get(&self) -> &V {
        &self.entries[self.index].1
    }
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.entries[self.index].1
    }
    pub fn into_mut(self) -> &'a mut V {
        &mut self.entries[self.index].1
    }
    pub fn insert(&mut self, value: V) -> V {
        core::mem::replace(&mut self.entries[self.index].1, value)
    }
    pub fn remove(self) -> V {
        let removed = self.entries.remove(self.index);
        removed.1
    }
}
impl<'a, K, V> LrrVacantEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn into_key(self) -> K {
        self.key
    }
    pub fn insert(self, value: V) -> &'a mut V {
        self.entries.insert(self.index, (self.key, value));
        &mut self.entries[self.index].1
    }
}

pub struct LrrIter<'a, K, V> {
    inner: core::slice::Iter<'a, (K, V)>,
}
impl<'a, K, V> Iterator for LrrIter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some((&entry.0, &entry.1)),
            Option::None => Option::None,
        }
    }
}
impl<'a, K, V> core::iter::DoubleEndedIterator for LrrIter<'a, K, V> {
    fn next_back(&mut self) -> Option<(&'a K, &'a V)> {
        match self.inner.next_back() {
            Option::Some(entry) => Option::Some((&entry.0, &entry.1)),
            Option::None => Option::None,
        }
    }
}
pub struct LrrIterMut<'a, K, V> {
    inner: core::slice::IterMut<'a, (K, V)>,
}
impl<'a, K, V> Iterator for LrrIterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some((&entry.0, &mut entry.1)),
            Option::None => Option::None,
        }
    }
}
pub struct LrrKeys<'a, K, V> {
    inner: core::slice::Iter<'a, (K, V)>,
}
impl<'a, K, V> Iterator for LrrKeys<'a, K, V> {
    type Item = &'a K;
    fn next(&mut self) -> Option<&'a K> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some(&entry.0),
            Option::None => Option::None,
        }
    }
}
pub struct LrrValues<'a, K, V> {
    inner: core::slice::Iter<'a, (K, V)>,
}
impl<'a, K, V> Iterator for LrrValues<'a, K, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<&'a V> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some(&entry.1),
            Option::None => Option::None,
        }
    }
}
pub struct LrrValuesMut<'a, K, V> {
    inner: core::slice::IterMut<'a, (K, V)>,
}
impl<'a, K, V> Iterator for LrrValuesMut<'a, K, V> {
    type Item = &'a mut V;
    fn next(&mut self) -> Option<&'a mut V> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some(&mut entry.1),
            Option::None => Option::None,
        }
    }
}
pub struct LrrIntoKeys<K, V> {
    inner: crate::vec::IntoIter<(K, V)>,
}
impl<K, V> Iterator for LrrIntoKeys<K, V> {
    type Item = K;
    fn next(&mut self) -> Option<K> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
}
pub struct LrrIntoValues<K, V> {
    inner: crate::vec::IntoIter<(K, V)>,
}
impl<K, V> Iterator for LrrIntoValues<K, V> {
    type Item = V;
    fn next(&mut self) -> Option<V> {
        match self.inner.next() {
            Option::Some(entry) => Option::Some(entry.1),
            Option::None => Option::None,
        }
    }
}

pub struct BTreeMap<K, V> {
    map: LrrMap<K, V>,
}
pub struct BTreeSet<T> {
    map: LrrMap<T, ()>,
}
pub struct VecDeque<T> {
    buf: Vec<T>,
}
pub struct BinaryHeap<T> {
    data: Vec<T>,
}
pub struct LinkedList<T> {
    marker: core::marker::PhantomData<T>,
}

impl<K, V> BTreeMap<K, V> {
    pub const fn new() -> BTreeMap<K, V> {
        BTreeMap {
            map: LrrMap::new(true),
        }
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Ord,
    {
        self.map.insert(key, value)
    }
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    {
        self.map.get(key)
    }
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    {
        self.map.get_mut(key)
    }
    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    {
        self.map.get_key_value(key)
    }
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    {
        self.map.contains_key(key)
    }
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    {
        self.map.remove(key)
    }
    pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    {
        self.map.remove_entry(key)
    }
    pub fn entry(&mut self, key: K) -> btree_map::Entry<'_, K, V>
    where
        K: Ord,
    {
        self.map.entry(key)
    }
    pub fn iter(&self) -> btree_map::Iter<'_, K, V> {
        self.map.iter()
    }
    pub fn iter_mut(&mut self) -> btree_map::IterMut<'_, K, V> {
        self.map.iter_mut()
    }
    pub fn keys(&self) -> btree_map::Keys<'_, K, V> {
        self.map.keys()
    }
    pub fn values(&self) -> btree_map::Values<'_, K, V> {
        self.map.values()
    }
    pub fn values_mut(&mut self) -> btree_map::ValuesMut<'_, K, V> {
        self.map.values_mut()
    }
    pub fn into_keys(self) -> btree_map::IntoKeys<K, V> {
        self.map.into_keys()
    }
    pub fn into_values(self) -> btree_map::IntoValues<K, V> {
        self.map.into_values()
    }
    pub fn retain<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F) {
        self.map.retain(f)
    }
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        self.map.first_key_value()
    }
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        self.map.last_key_value()
    }
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        self.map.pop_first()
    }
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        self.map.pop_last()
    }
}
impl<K: Clone, V: Clone> Clone for BTreeMap<K, V> {
    fn clone(&self) -> BTreeMap<K, V> {
        BTreeMap {
            map: self.map.clone(),
        }
    }
}
impl<K, V> Default for BTreeMap<K, V> {
    fn default() -> BTreeMap<K, V> {
        BTreeMap::new()
    }
}
impl<K: Ord, V: PartialEq> PartialEq for BTreeMap<K, V> {
    fn eq(&self, other: &BTreeMap<K, V>) -> bool {
        self.map == other.map
    }
}
impl<K: Ord, V: Eq> Eq for BTreeMap<K, V> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for BTreeMap<K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut map = f.debug_map();
        for (key, value) in self.map.iter() {
            map.entry(key, value);
        }
        map.finish()
    }
}
impl<K, V> IntoIterator for BTreeMap<K, V> {
    type Item = (K, V);
    type IntoIter = crate::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> crate::vec::IntoIter<(K, V)> {
        self.map.lrr_into_entries().into_iter()
    }
}
impl<'a, K, V> IntoIterator for &'a BTreeMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = LrrIter<'a, K, V>;
    fn into_iter(self) -> LrrIter<'a, K, V> {
        self.map.iter()
    }
}
impl<'a, K, V> IntoIterator for &'a mut BTreeMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = LrrIterMut<'a, K, V>;
    fn into_iter(self) -> LrrIterMut<'a, K, V> {
        self.map.iter_mut()
    }
}
impl<K: Ord, V> core::iter::FromIterator<(K, V)> for BTreeMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> BTreeMap<K, V> {
        let mut map = BTreeMap::<K, V>::new();
        let mut it = iter.into_iter();
        while let Option::Some(pair) = it.next() {
            map.insert(pair.0, pair.1);
        }
        map
    }
}
impl<K: Ord, V> core::iter::Extend<(K, V)> for BTreeMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Option::Some(pair) = it.next() {
            self.insert(pair.0, pair.1);
        }
    }
}

impl<T> BTreeSet<T> {
    pub const fn new() -> BTreeSet<T> {
        BTreeSet {
            map: LrrMap::new(true),
        }
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    pub fn insert(&mut self, value: T) -> bool
    where
        T: Ord,
    {
        match self.map.insert(value, ()) {
            Option::Some(_) => false,
            Option::None => true,
        }
    }
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    {
        self.map.contains_key(value)
    }
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    {
        match self.map.remove(value) {
            Option::Some(_) => true,
            Option::None => false,
        }
    }
    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    {
        match self.map.remove_entry(value) {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    {
        match self.map.get_key_value(value) {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn iter(&self) -> btree_map::Keys<'_, T, ()> {
        self.map.keys()
    }
    pub fn first(&self) -> Option<&T> {
        match self.map.first_key_value() {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn last(&self) -> Option<&T> {
        match self.map.last_key_value() {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn pop_first(&mut self) -> Option<T> {
        match self.map.pop_first() {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn pop_last(&mut self) -> Option<T> {
        match self.map.pop_last() {
            Option::Some(entry) => Option::Some(entry.0),
            Option::None => Option::None,
        }
    }
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, mut f: F) {
        self.map.retain(|key, _| f(key))
    }
    pub fn is_subset(&self, other: &BTreeSet<T>) -> bool
    where
        T: Ord,
    {
        for item in self.iter() {
            if !other.contains(item) {
                return false;
            }
        }
        true
    }
    pub fn is_superset(&self, other: &BTreeSet<T>) -> bool
    where
        T: Ord,
    {
        other.is_subset(self)
    }
    pub fn is_disjoint(&self, other: &BTreeSet<T>) -> bool
    where
        T: Ord,
    {
        for item in self.iter() {
            if other.contains(item) {
                return false;
            }
        }
        true
    }
    pub fn union<'a>(&'a self, other: &'a BTreeSet<T>) -> crate::vec::IntoIter<&'a T>
    where
        T: Ord,
    {
        let mut out = lrr_ref_vec::<T>();
        let mut a = self.iter();
        let mut b = other.iter();
        let mut next_a = a.next();
        let mut next_b = b.next();
        loop {
            match (next_a, next_b) {
                (Option::Some(x), Option::Some(y)) => match x.cmp(y) {
                    Ordering::Less => {
                        out.push(x);
                        next_a = a.next();
                    }
                    Ordering::Greater => {
                        out.push(y);
                        next_b = b.next();
                    }
                    Ordering::Equal => {
                        out.push(x);
                        next_a = a.next();
                        next_b = b.next();
                    }
                },
                (Option::Some(x), Option::None) => {
                    out.push(x);
                    next_a = a.next();
                }
                (Option::None, Option::Some(y)) => {
                    out.push(y);
                    next_b = b.next();
                }
                (Option::None, Option::None) => break,
            }
        }
        out.into_iter()
    }
    pub fn intersection<'a>(&'a self, other: &'a BTreeSet<T>) -> crate::vec::IntoIter<&'a T>
    where
        T: Ord,
    {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            if other.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
    pub fn difference<'a>(&'a self, other: &'a BTreeSet<T>) -> crate::vec::IntoIter<&'a T>
    where
        T: Ord,
    {
        let mut out = lrr_ref_vec::<T>();
        for item in self.iter() {
            if !other.contains(item) {
                out.push(item);
            }
        }
        out.into_iter()
    }
}
impl<T: Clone> Clone for BTreeSet<T> {
    fn clone(&self) -> BTreeSet<T> {
        BTreeSet {
            map: self.map.clone(),
        }
    }
}
impl<T> Default for BTreeSet<T> {
    fn default() -> BTreeSet<T> {
        BTreeSet::new()
    }
}
impl<T: Ord> PartialEq for BTreeSet<T> {
    fn eq(&self, other: &BTreeSet<T>) -> bool {
        self.map == other.map
    }
}
impl<T: Ord> Eq for BTreeSet<T> {}
impl<T: core::fmt::Debug> core::fmt::Debug for BTreeSet<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut set = f.debug_set();
        for item in self.map.keys() {
            set.entry(item);
        }
        set.finish()
    }
}
impl<T> IntoIterator for BTreeSet<T> {
    type Item = T;
    type IntoIter = LrrIntoKeys<T, ()>;
    fn into_iter(self) -> LrrIntoKeys<T, ()> {
        self.map.into_keys()
    }
}
impl<'a, T> IntoIterator for &'a BTreeSet<T> {
    type Item = &'a T;
    type IntoIter = LrrKeys<'a, T, ()>;
    fn into_iter(self) -> LrrKeys<'a, T, ()> {
        self.map.keys()
    }
}
impl<T: Ord> core::iter::FromIterator<T> for BTreeSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> BTreeSet<T> {
        let mut set = BTreeSet::<T>::new();
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            set.insert(item);
        }
        set
    }
}
impl<T: Ord> core::iter::Extend<T> for BTreeSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            self.insert(item);
        }
    }
}

impl<T> VecDeque<T> {
    pub const fn new() -> VecDeque<T> {
        VecDeque { buf: Vec::new() }
    }
    pub fn with_capacity(capacity: usize) -> VecDeque<T> {
        VecDeque { buf: Vec::new() }
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.len() == 0
    }
    pub fn push_back(&mut self, value: T) {
        self.buf.push(value);
    }
    pub fn push_front(&mut self, value: T) {
        self.buf.insert(0, value);
    }
    pub fn pop_back(&mut self) -> Option<T> {
        self.buf.pop()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        if self.buf.len() == 0 {
            Option::None
        } else {
            Option::Some(self.buf.remove(0))
        }
    }
    pub fn front(&self) -> Option<&T> {
        self.buf.first()
    }
    pub fn back(&self) -> Option<&T> {
        self.buf.last()
    }
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.buf.first_mut()
    }
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.buf.last_mut()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.buf.get(index)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.buf.get_mut(index)
    }
    pub fn clear(&mut self) {
        self.buf.clear();
    }
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.buf.contains(value)
    }
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.buf.iter()
    }
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.buf.iter_mut()
    }
    pub fn make_contiguous(&mut self) -> &mut [T] {
        self.buf.as_mut_slice()
    }
    pub fn extend_from_slice(&mut self, other: &[T])
    where
        T: Clone,
    {
        self.buf.extend_from_slice(other);
    }
}
impl<T: Clone> Clone for VecDeque<T> {
    fn clone(&self) -> VecDeque<T> {
        VecDeque {
            buf: self.buf.clone(),
        }
    }
}
impl<T> Default for VecDeque<T> {
    fn default() -> VecDeque<T> {
        VecDeque::new()
    }
}
impl<T: PartialEq> PartialEq for VecDeque<T> {
    fn eq(&self, other: &VecDeque<T>) -> bool {
        self.buf == other.buf
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for VecDeque<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.buf, f)
    }
}
impl<T> core::ops::Index<usize> for VecDeque<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.buf[index]
    }
}
impl<T> core::ops::IndexMut<usize> for VecDeque<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.buf[index]
    }
}
impl<T> IntoIterator for VecDeque<T> {
    type Item = T;
    type IntoIter = crate::vec::IntoIter<T>;
    fn into_iter(self) -> crate::vec::IntoIter<T> {
        self.buf.into_iter()
    }
}
impl<'a, T> IntoIterator for &'a VecDeque<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> core::slice::Iter<'a, T> {
        self.buf.iter()
    }
}
impl<T> core::iter::FromIterator<T> for VecDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> VecDeque<T> {
        let mut deque = VecDeque::<T>::new();
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            deque.push_back(item);
        }
        deque
    }
}
impl<T> core::iter::Extend<T> for VecDeque<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            self.push_back(item);
        }
    }
}

impl<T: Ord> BinaryHeap<T> {
    pub fn new() -> BinaryHeap<T> {
        BinaryHeap { data: Vec::new() }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }
    // The vector is kept sorted ascending: the maximum is the last element.
    pub fn push(&mut self, item: T) {
        let mut low: usize = 0;
        let mut high: usize = self.data.len();
        while low < high {
            let mid: usize = low + (high - low) / 2;
            if self.data[mid].cmp(&item) == Ordering::Greater {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        self.data.insert(low, item);
    }
    pub fn pop(&mut self) -> Option<T> {
        self.data.pop()
    }
    pub fn peek(&self) -> Option<&T> {
        self.data.last()
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }
    pub fn into_sorted_vec(self) -> Vec<T> {
        self.data
    }
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }
}
impl<T: Ord> Default for BinaryHeap<T> {
    fn default() -> BinaryHeap<T> {
        BinaryHeap::new()
    }
}
impl<T: Ord> core::iter::FromIterator<T> for BinaryHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> BinaryHeap<T> {
        let mut heap = BinaryHeap::<T>::new();
        let mut it = iter.into_iter();
        while let Option::Some(item) = it.next() {
            heap.push(item);
        }
        heap
    }
}

pub mod btree_map {
    pub use super::BTreeMap;
    pub use super::LrrEntry as Entry;
    pub use super::LrrIntoKeys as IntoKeys;
    pub use super::LrrIntoValues as IntoValues;
    pub use super::LrrIter as Iter;
    pub use super::LrrIterMut as IterMut;
    pub use super::LrrKeys as Keys;
    pub use super::LrrOccupiedEntry as OccupiedEntry;
    pub use super::LrrValues as Values;
    pub use super::LrrValuesMut as ValuesMut;
    pub use super::LrrVacantEntry as VacantEntry;
}
pub mod btree_set {
    pub use super::BTreeSet;
}
pub mod vec_deque {
    pub use super::VecDeque;
}
pub mod binary_heap {
    pub use super::BinaryHeap;
}
pub mod linked_list {
    pub use super::LinkedList;
}

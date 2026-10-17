// `std::collections`.
pub use alloc_crate::collections::{
    binary_heap, btree_map, btree_set, linked_list, vec_deque, BTreeMap, BTreeSet, BinaryHeap,
    LinkedList, VecDeque,
};

pub struct HashMap<K, V, S = hash_map::RandomState> {
    entries: alloc_crate::vec::Vec<(K, V)>,
    hash_builder: S,
}
pub struct HashSet<T, S = hash_map::RandomState> {
    map: HashMap<T, (), S>,
}

impl<K, V> HashMap<K, V, hash_map::RandomState> {
    pub fn new() -> HashMap<K, V, hash_map::RandomState> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::new")
    }
    pub fn with_capacity(capacity: usize) -> HashMap<K, V, hash_map::RandomState> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::with_capacity")
    }
}
impl<K, V, S> HashMap<K, V, S> {
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::insert")
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::get")
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::get_mut")
    }
    pub fn contains_key(&self, k: &K) -> bool {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::contains_key")
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::remove")
    }
    pub fn len(&self) -> usize {
        core::intrinsics::lrr_unsupported("std::collections::HashMap::len")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
impl<T> HashSet<T, hash_map::RandomState> {
    pub fn new() -> HashSet<T, hash_map::RandomState> {
        core::intrinsics::lrr_unsupported("std::collections::HashSet::new")
    }
}
impl<T, S> HashSet<T, S> {
    pub fn insert(&mut self, value: T) -> bool {
        core::intrinsics::lrr_unsupported("std::collections::HashSet::insert")
    }
    pub fn contains(&self, value: &T) -> bool {
        core::intrinsics::lrr_unsupported("std::collections::HashSet::contains")
    }
    pub fn len(&self) -> usize {
        core::intrinsics::lrr_unsupported("std::collections::HashSet::len")
    }
}

pub mod hash_map {
    pub use super::HashMap;
    pub struct RandomState {
        k0: u64,
        k1: u64,
    }
    pub struct DefaultHasher {
        state: u64,
    }
    pub enum Entry<'a, K: 'a, V: 'a> {
        Occupied(OccupiedEntry<'a, K, V>),
        Vacant(VacantEntry<'a, K, V>),
    }
    pub struct OccupiedEntry<'a, K, V> {
        key: &'a K,
        value: &'a mut V,
    }
    pub struct VacantEntry<'a, K, V> {
        key: K,
        map: &'a mut V,
    }
}
pub mod hash_set {
    pub use super::HashSet;
}

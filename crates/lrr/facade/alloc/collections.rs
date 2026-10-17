// `alloc::collections`.
pub struct BTreeMap<K, V> {
    entries: crate::vec::Vec<(K, V)>,
}
pub struct BTreeSet<T> {
    map: BTreeMap<T, ()>,
}
pub struct VecDeque<T> {
    buf: crate::vec::Vec<T>,
}
pub struct BinaryHeap<T> {
    data: crate::vec::Vec<T>,
}
pub struct LinkedList<T> {
    marker: core::marker::PhantomData<T>,
}

impl<K, V> BTreeMap<K, V> {
    pub const fn new() -> BTreeMap<K, V> {
        core::intrinsics::lrr_unsupported("alloc::collections::BTreeMap::new")
    }
}
impl<T> BTreeSet<T> {
    pub const fn new() -> BTreeSet<T> {
        core::intrinsics::lrr_unsupported("alloc::collections::BTreeSet::new")
    }
}
impl<T> VecDeque<T> {
    pub const fn new() -> VecDeque<T> {
        core::intrinsics::lrr_unsupported("alloc::collections::VecDeque::new")
    }
}

pub mod btree_map {
    pub use super::BTreeMap;
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

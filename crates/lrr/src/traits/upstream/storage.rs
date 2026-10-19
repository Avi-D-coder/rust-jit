//! Context-owned structural interning. Handles are temporary upstream IR
//! representations, never portable source identities or persistent cache keys.
use std::{
    cell::RefCell,
    collections::HashSet,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};
use typed_arena::Arena;

/// A shared immutable node owned by one borrowed solver context. Copying a
/// handle neither clones a type tree nor extends that context's lifetime.
pub(crate) struct Interned<'ctx, T>(&'ctx T);
impl<T> Copy for Interned<'_, T> {}
impl<T> Clone for Interned<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for Interned<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}
impl<T> Eq for Interned<'_, T> {}
impl<T> Hash for Interned<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::from_ref(self.0).hash(state);
    }
}
impl<T: fmt::Debug> fmt::Debug for Interned<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<T> Deref for Interned<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}
impl<'ctx, T> Interned<'ctx, T> {
    pub(crate) fn value(self) -> &'ctx T {
        self.0
    }
}
impl<I, T: Copy> rustc_type_ir::intern::Interned<I> for Interned<'_, T> {
    type Value = T;
    fn get(self) -> T {
        *self.0
    }
}

/// The structural index is cold preparation state. Eq/Hash on returned handles
/// are constant-time; source/answer conversion must use portable source keys.
/// Index entries drop before the arena, and the arena runs node destructors.
pub(crate) struct InternPool<'ctx, T> {
    index: RefCell<HashSet<&'ctx T>>,
    arena: Arena<T>,
}
impl<T> Default for InternPool<'_, T> {
    fn default() -> Self {
        Self {
            index: RefCell::new(HashSet::new()),
            arena: Arena::new(),
        }
    }
}
impl<'ctx, T: Eq + Hash> InternPool<'ctx, T> {
    pub(crate) fn intern(&'ctx self, value: T) -> Interned<'ctx, T> {
        let existing = self.index.borrow().get(&value).copied();
        if let Some(existing) = existing {
            return Interned(existing);
        }
        let node: &'ctx T = self.arena.alloc(value);
        self.index.borrow_mut().insert(node);
        Interned(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    /// LRR ownership regression, not a miniature Rust language fixture.
    /// A destructor-bearing host node must be released with its solver context.
    struct Payload {
        key: u32,
        drops: Rc<Cell<u32>>,
    }
    impl PartialEq for Payload {
        fn eq(&self, rhs: &Self) -> bool {
            self.key == rhs.key
        }
    }
    impl Eq for Payload {}
    impl Hash for Payload {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.key.hash(h);
        }
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn nodes_share_by_structure_and_owned_context_drops_every_payload() {
        let drops = Rc::new(Cell::new(0));
        {
            let pool = InternPool::default();
            let first = pool.intern(Payload {
                key: 7,
                drops: Rc::clone(&drops),
            });
            let same = pool.intern(Payload {
                key: 7,
                drops: Rc::clone(&drops),
            });
            let other = pool.intern(Payload {
                key: 9,
                drops: Rc::clone(&drops),
            });
            assert!(first == same);
            assert!(first != other);
            assert_eq!(drops.get(), 1, "unused duplicate is destroyed immediately");
            assert_eq!(first.key, 7);
            assert_eq!(other.key, 9);
        }
        assert_eq!(drops.get(), 3, "both retained nodes die with the context");
    }

    #[test]
    fn handles_are_context_local_and_implement_the_actual_upstream_interned_api() {
        let a = InternPool::default();
        let b = InternPool::default();
        let left = a.intern((3u32, 5u32));
        let same = a.intern((3u32, 5u32));
        let foreign = b.intern((3u32, 5u32));
        assert_eq!(left, same);
        assert_ne!(left, foreign);
        assert_eq!(
            <Interned<'_, (u32, u32)> as rustc_type_ir::intern::Interned<()>>::get(left),
            (3, 5)
        );
        assert_eq!(*left.value(), *foreign.value());
    }
}

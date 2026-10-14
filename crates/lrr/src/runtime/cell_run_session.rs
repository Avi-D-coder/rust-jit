//! Process-local store incarnation, not a guest handle or recipe identity.
use crate::core::diagnostic::Diagnostic;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(1);
/// No public constructor, stable encoding, serialization or numeric accessor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct StoreSession(u64);
impl StoreSession {
    // This child is owned by GuestStore; callers cannot supply an identity.
    pub(super) fn mint() -> Result<Self, Diagnostic> {
        NEXT.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map(Self)
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-RESOURCE-LIMIT",
                    "store session identity exhausted".into(),
                )
            })
    }
}
#[cfg(test)]
mod tests {
    use super::super::GuestStore;
    #[test]
    fn stores_with_identical_arena_handle_bits_have_distinct_actual_sessions() {
        let mut a = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut b = GuestStore::new(1, 1000, 1_000_000).unwrap();
        assert_eq!(
            a.new_frame_anchor().unwrap().bits(),
            b.new_frame_anchor().unwrap().bits()
        );
        assert_ne!(a.pattern_binding_session(), b.pattern_binding_session());
    }
}

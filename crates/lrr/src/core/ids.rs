//! Worker-local identities must never be restored from portable input.
//!
//! Identity families cannot be mixed, even if their indexes happen to match:
//! ```compile_fail
//! use lrr::core::ids::{BodyId, DefId, IdAllocator};
//! let definition: DefId = IdAllocator::<DefId>::default().allocate().unwrap();
//! let body: BodyId = definition;
//! ```
use super::diagnostic::Diagnostic;
use std::{marker::PhantomData, num::NonZeroU32};

mod sealed {
    pub trait Sealed {}
}
pub trait WorkerId: sealed::Sealed + Copy + Ord {
    fn from_index(index: NonZeroU32) -> Self;
    fn index(self) -> NonZeroU32;
}
macro_rules! worker_ids {
    ($($name:ident),* $(,)?) => { $(
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(NonZeroU32);
        impl sealed::Sealed for $name {}
        impl WorkerId for $name {
            fn from_index(index: NonZeroU32) -> Self { Self(index) }
            fn index(self) -> NonZeroU32 { self.0 }
        }
    )* };
}
worker_ids!(
    DefId, BodyId, TypeId, RecipeId, OpId, PcId, ScopeId, SlotId, FrameId
);

/// Effect: allocates a monotonic worker identity, never reuses or wraps it.
pub struct IdAllocator<I: WorkerId> {
    next: Option<NonZeroU32>,
    kind: PhantomData<I>,
}
impl<I: WorkerId> Default for IdAllocator<I> {
    fn default() -> Self {
        Self {
            next: NonZeroU32::new(1),
            kind: PhantomData,
        }
    }
}
impl<I: WorkerId> IdAllocator<I> {
    pub fn allocate(&mut self) -> Result<I, Diagnostic> {
        let index = self.next.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-ID-EXHAUSTED",
                "worker identity exhausted; retire worker between runs".into(),
            )
        })?;
        self.next = index.get().checked_add(1).and_then(NonZeroU32::new);
        Ok(I::from_index(index))
    }
}

macro_rules! guest_ids {
    ($($name:ident),* $(,)?) => { $(
        /// Guest identity with run epoch and arena index, both nonzero.
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(u64);
        impl $name {
            pub fn from_parts(epoch: NonZeroU32, index: NonZeroU32) -> Self {
                Self((u64::from(epoch.get()) << 32) | u64::from(index.get()))
            }
            pub fn epoch(self) -> u32 { (self.0 >> 32) as u32 }
            pub fn index(self) -> u32 { self.0 as u32 }
            pub fn bits(self) -> u64 { self.0 }
        }
    )* };
}
guest_ids!(CellId, ObjectId, RefId);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_ids_are_nonzero_monotonic_and_do_not_wrap() {
        let mut ids = IdAllocator::<DefId>::default();
        assert_eq!(ids.allocate().unwrap().index().get(), 1);
        assert_eq!(ids.allocate().unwrap().index().get(), 2);
        ids.next = NonZeroU32::new(u32::MAX);
        assert_eq!(ids.allocate().unwrap().index().get(), u32::MAX);
        assert_eq!(ids.allocate().unwrap_err().code, "LRR-E-ID-EXHAUSTED");
        assert!(ids.allocate().is_err());
    }
    #[test]
    fn guest_identity_retains_epoch_and_index() {
        let cell = CellId::from_parts(NonZeroU32::new(7).unwrap(), NonZeroU32::new(11).unwrap());
        assert_eq!(cell.epoch(), 7);
        assert_eq!(cell.index(), 11);
        assert_eq!(cell.bits(), (7_u64 << 32) | 11);
    }
}

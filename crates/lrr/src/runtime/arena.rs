//! Typed append-only run arenas. Index zero and tombstones never become live.
use crate::core::diagnostic::Diagnostic;
use std::marker::PhantomData;
pub(crate) fn stale() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-STALE-HANDLE",
        "wrong run epoch, arena index, or retired handle".into(),
    )
}
pub(crate) fn resource() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-RUNTIME-RESOURCE",
        "guest arena handle/memory reservation exhausted".into(),
    )
}
pub struct Handle<T> {
    bits: u64,
    marker: PhantomData<fn() -> T>,
}
impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Handle({:#018x})", self.bits)
    }
}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}
impl<T> Eq for Handle<T> {}
impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.bits.cmp(&other.bits)
    }
}
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Handle<T> {
    pub fn bits(self) -> u64 {
        self.bits
    }
    pub(crate) fn from_bits(bits: u64) -> Self {
        Self {
            bits,
            marker: PhantomData,
        }
    }
}
/// Run epoch plus unlimited allocation observations. E-R5-006: rustc has no
/// guest handle or memory limit; the OS bounds memory, so nothing here refuses.
#[derive(Debug)]
pub struct RunBudget {
    epoch: u32,
    handles: u32,
    bytes: usize,
}
impl RunBudget {
    // E-R5-006: ignored, remove at callers (`_max_handles`, `_max_bytes`).
    pub fn new(epoch: u32, _max_handles: u32, _max_bytes: usize) -> Result<Self, Diagnostic> {
        Self::for_epoch(epoch)
    }
    pub fn for_epoch(epoch: u32) -> Result<Self, Diagnostic> {
        if epoch == 0 {
            return Err(stale());
        }
        Ok(Self {
            epoch,
            handles: 0,
            bytes: 0,
        })
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    /// Cumulative handles allocated in this run (observation only).
    pub fn handles(&self) -> u32 {
        self.handles
    }
    /// Currently owned guest bytes (observation only).
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn charge(&mut self, handles: u32, bytes: usize) {
        self.handles = self.handles.saturating_add(handles);
        self.bytes = self.bytes.saturating_add(bytes);
    }
    fn release(&mut self, bytes: usize) {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .expect("owned guest allocation was charged");
    }
}
pub struct GuestArena<T> {
    epoch: u32,
    entries: Vec<Option<T>>,
    owned_bytes: Vec<usize>,
}
impl<T> GuestArena<T> {
    pub fn new(epoch: u32) -> Result<Self, Diagnostic> {
        if epoch == 0 {
            return Err(stale());
        }
        Ok(Self {
            epoch,
            entries: vec![None],
            owned_bytes: vec![0],
        })
    }
    /// Empty arena metadata for an owner that reserves all capacity at first use.
    /// Unlike the legacy constructor, even the sentinel buffers are deferred
    /// until reserve.
    pub(crate) fn new_lazy(epoch: u32) -> Result<Self, Diagnostic> {
        if epoch == 0 {
            return Err(stale());
        }
        Ok(Self {
            epoch,
            entries: Vec::new(),
            owned_bytes: Vec::new(),
        })
    }
    fn initialize_lazy(&mut self, count: usize, budget: &mut RunBudget) -> Result<(), Diagnostic> {
        // Handle indexes are u32 (Handle encoding), not a policy limit.
        let total = count
            .checked_add(1)
            .filter(|n| *n <= u32::MAX as usize)
            .ok_or_else(resource)?;
        // Keep both temporary buffers local until both allocations succeed.
        let mut entries = Vec::new();
        entries.try_reserve_exact(total).map_err(|_| resource())?;
        let mut sizes = Vec::new();
        sizes.try_reserve_exact(total).map_err(|_| resource())?;
        let actual = entries
            .capacity()
            .checked_mul(std::mem::size_of::<Option<T>>())
            .and_then(|n| {
                sizes
                    .capacity()
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(resource)?;
        budget.charge(0, actual);
        entries.push(None);
        sizes.push(0);
        self.entries = entries;
        self.owned_bytes = sizes;
        Ok(())
    }
    /// Check that `count` more entries fit the u32 handle index space (the
    /// Handle encoding, not a resource policy) without allocating.
    pub(crate) fn admits(&self, count: usize) -> Result<(), Diagnostic> {
        self.entries
            .len()
            .max(1) // A lazy arena still reserves its unaddressable sentinel.
            .checked_add(count)
            .filter(|n| *n <= u32::MAX as usize)
            .map(|_| ())
            .ok_or_else(resource)
    }
    /// Reserve all replacement buffers before moving any retained record. A
    /// failure may not leave an uncharged allocator-grown buffer in the arena.
    pub(crate) fn reserve_atomic(
        &mut self,
        count: usize,
        budget: &mut RunBudget,
    ) -> Result<(), Diagnostic> {
        if self.epoch != budget.epoch {
            return Err(stale());
        }
        if self.entries.is_empty() {
            return self.initialize_lazy(count, budget);
        }
        let end = self
            .entries
            .len()
            .checked_add(count)
            .filter(|n| *n <= u32::MAX as usize)
            .ok_or_else(resource)?;
        // Geometric growth keeps repeated single-entry reservations amortized.
        let mut entries = if end > self.entries.capacity() {
            let mut values = Vec::new();
            values
                .try_reserve_exact(end.max(self.entries.capacity().saturating_mul(2)))
                .map_err(|_| resource())?;
            Some(values)
        } else {
            None
        };
        let mut sizes = if end > self.owned_bytes.capacity() {
            let mut values = Vec::new();
            values
                .try_reserve_exact(end.max(self.owned_bytes.capacity().saturating_mul(2)))
                .map_err(|_| resource())?;
            Some(values)
        } else {
            None
        };
        let entry_delta = entries
            .as_ref()
            .map_or(Some(0), |v| {
                v.capacity()
                    .checked_sub(self.entries.capacity())
                    .and_then(|n| n.checked_mul(std::mem::size_of::<Option<T>>()))
            })
            .ok_or_else(resource)?;
        let size_delta = sizes
            .as_ref()
            .map_or(Some(0), |v| {
                v.capacity()
                    .checked_sub(self.owned_bytes.capacity())
                    .and_then(|n| n.checked_mul(std::mem::size_of::<usize>()))
            })
            .ok_or_else(resource)?;
        budget.charge(0, entry_delta.checked_add(size_delta).ok_or_else(resource)?);
        // No fallible allocation or guest callback follows the
        // first move. Capacities include all old entries and the incoming batch.
        if let Some(mut values) = entries.take() {
            values.append(&mut self.entries);
            self.entries = values;
        }
        if let Some(mut values) = sizes.take() {
            values.append(&mut self.owned_bytes);
            self.owned_bytes = values;
        }
        Ok(())
    }
    pub fn reserve(&mut self, count: usize, budget: &mut RunBudget) -> Result<(), Diagnostic> {
        if self.epoch != budget.epoch {
            return Err(stale());
        }
        if self.entries.is_empty() {
            return self.initialize_lazy(count, budget);
        }
        self.entries
            .len()
            .checked_add(count)
            .filter(|n| *n <= u32::MAX as usize)
            .ok_or_else(resource)?;
        let old_entries = self.entries.capacity();
        let old_sizes = self.owned_bytes.capacity();
        // Amortized growth: exact per-allocation reservation existed only for
        // byte accounting and made append-only arenas quadratic.
        self.entries.try_reserve(count).map_err(|_| resource())?;
        let entry_charge = (self.entries.capacity() - old_entries)
            .checked_mul(std::mem::size_of::<Option<T>>())
            .ok_or_else(resource)?;
        budget.charge(0, entry_charge);
        self.owned_bytes
            .try_reserve(count)
            .map_err(|_| resource())?;
        let size_charge = (self.owned_bytes.capacity() - old_sizes)
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or_else(resource)?;
        budget.charge(0, size_charge);
        Ok(())
    }
    pub fn allocate(
        &mut self,
        value: T,
        owned_bytes: usize,
        budget: &mut RunBudget,
    ) -> Result<Handle<T>, Diagnostic> {
        self.reserve(1, budget)?;
        budget.charge(1, owned_bytes);
        let index = u32::try_from(self.entries.len()).map_err(|_| resource())?;
        self.entries.push(Some(value));
        self.owned_bytes.push(owned_bytes);
        Ok(Handle::from_bits(
            (u64::from(self.epoch) << 32) | u64::from(index),
        ))
    }
    fn index(&self, handle: Handle<T>) -> Result<usize, Diagnostic> {
        let epoch = (handle.bits >> 32) as u32;
        let index = handle.bits as u32;
        if epoch != self.epoch
            || index == 0
            || self.entries.get(index as usize).is_none_or(Option::is_none)
        {
            return Err(stale());
        }
        Ok(index as usize)
    }
    pub fn get(&self, handle: impl Into<Handle<T>>) -> Result<&T, Diagnostic> {
        Ok(self.entries[self.index(handle.into())?]
            .as_ref()
            .expect("occupied arena entry"))
    }
    pub(crate) fn get_mut(&mut self, handle: impl Into<Handle<T>>) -> Result<&mut T, Diagnostic> {
        let index = self.index(handle.into())?;
        Ok(self.entries[index].as_mut().expect("occupied arena entry"))
    }
    pub fn take(
        &mut self,
        handle: impl Into<Handle<T>>,
        budget: &mut RunBudget,
    ) -> Result<T, Diagnostic> {
        if budget.epoch != self.epoch {
            return Err(stale());
        }
        let index = self.index(handle.into())?;
        let value = self.entries[index].take().expect("occupied arena entry");
        budget.release(self.owned_bytes[index]);
        self.owned_bytes[index] = 0;
        Ok(value)
    }
    pub fn reset(&mut self, new_epoch: u32) -> Result<(), Diagnostic> {
        if new_epoch <= self.epoch {
            return Err(stale());
        }
        self.entries = vec![None];
        self.owned_bytes = vec![0];
        self.epoch = new_epoch;
        Ok(())
    }
}

/// Arena handles have distinct compile-time record types.
/// ```compile_fail
/// use lrr::runtime::arena::{GuestArena,RunBudget};
/// let mut budget=RunBudget::new(1,10,10000).unwrap();
/// let mut cells=GuestArena::<u8>::new(1).unwrap();
/// let mut objects=GuestArena::<u16>::new(1).unwrap();
/// let cell=cells.allocate(1,0,&mut budget).unwrap();
/// objects.get(cell).unwrap();
/// ```
pub struct TypedArenaContract;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_decoding_rejects_zero_wrong_epoch_bounds_and_tombstones() {
        let mut budget = RunBudget::new(7, 10, 10000).unwrap();
        let mut arena = GuestArena::<u8>::new(7).unwrap();
        let id = arena.allocate(3, 8, &mut budget).unwrap();
        for raw in [0, 7u64 << 32, (6u64 << 32) | 1, (7u64 << 32) | 100] {
            assert_eq!(
                arena.get(Handle::<u8>::from_bits(raw)).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
        }
        let before = budget.bytes();
        assert_eq!(arena.take(id, &mut budget).unwrap(), 3);
        assert_eq!(budget.bytes(), before - 8);
        assert!(arena.get(id).is_err());
        let next = arena.allocate(4, 0, &mut budget).unwrap();
        assert_ne!(next.bits(), id.bits());
        assert_eq!(budget.handles(), 2);
    }
}

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
#[derive(Debug)]
pub struct RunBudget {
    epoch: u32,
    max_handles: u32,
    max_bytes: usize,
    handles: u32,
    bytes: usize,
}
impl RunBudget {
    pub fn new(epoch: u32, max_handles: u32, max_bytes: usize) -> Result<Self, Diagnostic> {
        if epoch == 0 || max_handles > 8_000_000 || max_bytes > 1024 * 1024 * 1024 {
            return Err(resource());
        }
        Ok(Self {
            epoch,
            max_handles,
            max_bytes,
            handles: 0,
            bytes: 0,
        })
    }
    pub(crate) fn max_handles(&self) -> u32 {
        self.max_handles
    }
    pub(crate) fn max_bytes(&self) -> usize {
        self.max_bytes
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn handles(&self) -> u32 {
        self.handles
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn check(&self, handles: u32, bytes: usize) -> Result<(), Diagnostic> {
        if self
            .handles
            .checked_add(handles)
            .is_none_or(|n| n > self.max_handles)
            || self
                .bytes
                .checked_add(bytes)
                .is_none_or(|n| n > self.max_bytes)
        {
            Err(resource())
        } else {
            Ok(())
        }
    }
    pub(crate) fn charge(&mut self, handles: u32, bytes: usize) -> Result<(), Diagnostic> {
        self.check(handles, bytes)?;
        self.handles += handles;
        self.bytes += bytes;
        Ok(())
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
    /// Predict the checked incremental arena capacity without allocation or
    /// logical publication, so a caller can admit a complete multi-arena batch.
    pub(crate) fn reservation_bytes(&self, count: usize) -> Result<usize, Diagnostic> {
        let end = self
            .entries
            .len()
            .checked_add(count)
            .filter(|n| *n <= u32::MAX as usize)
            .ok_or_else(resource)?;
        let new_entries = end.saturating_sub(self.entries.capacity());
        let new_sizes = end.saturating_sub(self.owned_bytes.capacity());
        new_entries
            .checked_mul(std::mem::size_of::<Option<T>>())
            .and_then(|n| {
                new_sizes
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(resource)
    }
    pub fn reserve(&mut self, count: usize, budget: &mut RunBudget) -> Result<(), Diagnostic> {
        if self.epoch != budget.epoch {
            return Err(stale());
        }
        let predicted = self.reservation_bytes(count)?;
        budget.check(0, predicted)?;
        let old_entries = self.entries.capacity();
        let old_sizes = self.owned_bytes.capacity();
        self.entries
            .try_reserve_exact(count)
            .map_err(|_| resource())?;
        let entry_charge = (self.entries.capacity() - old_entries)
            .checked_mul(std::mem::size_of::<Option<T>>())
            .ok_or_else(resource)?;
        budget.charge(0, entry_charge)?;
        self.owned_bytes
            .try_reserve_exact(count)
            .map_err(|_| resource())?;
        let size_charge = (self.owned_bytes.capacity() - old_sizes)
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or_else(resource)?;
        budget.charge(0, size_charge)?;
        Ok(())
    }
    pub fn allocate(
        &mut self,
        value: T,
        owned_bytes: usize,
        budget: &mut RunBudget,
    ) -> Result<Handle<T>, Diagnostic> {
        budget.check(1, owned_bytes)?;
        self.reserve(1, budget)?;
        budget.charge(1, owned_bytes)?;
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

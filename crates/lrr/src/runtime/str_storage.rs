//! Immutable run-owned UTF-8 backing. These storage APIs grant no source type,
//! literal, region, operator, method or executable recipe authority.
use super::*;
use crate::core::digest::Digest;
use crate::runtime::reference::BufferLease;
use std::{cmp::Ordering, ops::Range};

struct ImmutableStrBacking {
    // No mutation API: validity and immutability follow from the owned String.
    // This is backing storage, never a guest owned String value/descriptor.
    text: String,
}
/// Opaque run/store-owned backing handle, not a guest scalar or source proof.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ImmutableStrId {
    session: StoreSession,
    handle: Handle<ImmutableStrBacking>,
}
#[derive(Clone, Copy)]
struct PoolRow {
    digest: Digest,
    id: ImmutableStrId,
}
pub(super) struct ImmutableStrStorage {
    session: StoreSession,
    backings: GuestArena<ImmutableStrBacking>,
    // Sorted by digest then actual UTF-8 bytes. Hash collisions never establish
    // content identity. Every retained row/capacity is charged to RunBudget.
    rows: Vec<PoolRow>,
}
enum PreparedBacking {
    Existing(ImmutableStrId),
    New {
        digest: Digest,
        position: usize,
        backing: ImmutableStrBacking,
    },
}
/// Storage-only reservation; source/entry authority stays with the VM receipt.
pub(crate) struct PreparedImmutableStr {
    session: StoreSession,
    rows: usize,
    backing: PreparedBacking,
    ty: TypeId,
    range: Range<usize>,
    new: bool,
    owned: usize,
}
fn utf8_fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-STR-UTF8",
        "immutable str backing is not valid UTF-8".into(),
    )
}
fn range_fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-STR-RANGE",
        "str byte range is reversed, out of bounds or splits a UTF-8 character".into(),
    )
}
impl ImmutableStrStorage {
    pub(super) fn new(epoch: u32, session: StoreSession) -> Result<Self, Diagnostic> {
        Ok(Self {
            session,
            backings: GuestArena::new_lazy(epoch)?,
            rows: Vec::new(),
        })
    }
    fn backing(&self, id: ImmutableStrId) -> Result<&str, Diagnostic> {
        if id.session != self.session {
            return Err(stale());
        }
        Ok(self.backings.get(id.handle)?.text.as_str())
    }
    fn find(
        &self,
        digest: Digest,
        text: &str,
    ) -> Result<Result<ImmutableStrId, usize>, Diagnostic> {
        let (mut low, mut high) = (0, self.rows.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let row = self.rows[middle];
            let order = match row.digest.cmp(&digest) {
                Ordering::Equal => self.backing(row.id)?.as_bytes().cmp(text.as_bytes()),
                other => other,
            };
            match order {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(row.id)),
            }
        }
        Ok(Err(low))
    }
    fn reservation_bytes(&self, new: bool) -> Result<usize, Diagnostic> {
        if !new {
            return Ok(0);
        }
        self.backings
            .atomic_reservation_bytes(1)?
            .checked_add(if self.rows.len() == self.rows.capacity() {
                self.rows
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(std::mem::size_of::<PoolRow>()))
                    .ok_or_else(resource)?
            } else {
                0
            })
            .ok_or_else(resource)
    }
    fn reserve(&mut self, new: bool, budget: &mut RunBudget) -> Result<(), Diagnostic> {
        if !new {
            return Ok(());
        }
        self.backings.reserve_atomic(1, budget)?;
        if self.rows.len() == self.rows.capacity() {
            // Build temporary capacity first so allocator over-reservation or a
            // failed budget check cannot leave an uncharged live index buffer.
            let count = self.rows.len().checked_add(1).ok_or_else(resource)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count).map_err(|_| resource())?;
            let actual = rows
                .capacity()
                .checked_mul(std::mem::size_of::<PoolRow>())
                .ok_or_else(resource)?;
            // During replacement old and new index buffers coexist. Admit that
            // bounded transient peak before retaining the replacement capacity.
            budget.check(0, actual)?;
            let delta = rows
                .capacity()
                .checked_sub(self.rows.capacity())
                .and_then(|n| n.checked_mul(std::mem::size_of::<PoolRow>()))
                .ok_or_else(resource)?;
            rows.extend_from_slice(&self.rows);
            budget.charge(0, delta)?;
            self.rows = rows;
        }
        Ok(())
    }
    fn commit(
        &mut self,
        prepared: PreparedBacking,
        budget: &mut RunBudget,
    ) -> Result<ImmutableStrId, Diagnostic> {
        match prepared {
            PreparedBacking::Existing(id) => Ok(id),
            PreparedBacking::New {
                digest,
                position,
                backing,
            } => {
                let bytes = backing.text.capacity();
                let id = ImmutableStrId {
                    session: self.session,
                    handle: self.backings.allocate(backing, bytes, budget)?,
                };
                // Capacity and position were admitted before any arena publish;
                // no guest callback or storage mutation intervenes.
                self.rows.insert(position, PoolRow { digest, id });
                Ok(id)
            }
        }
    }
    pub(super) fn resolve(&self, lease: BufferLease) -> Result<&str, Diagnostic> {
        let text = self.backing(lease.backing)?;
        text.get(lease.start..lease.end).ok_or_else(range_fault)
    }
}
impl GuestStore {
    /// Storage primitive only. The VM has no emission path until its ordinary
    /// indexed literal/source certificate and frame publication join exists.
    pub fn immutable_str_ticket(&mut self, utf8: &[u8]) -> Result<TicketId, Diagnostic> {
        self.immutable_str_range_ticket(utf8, 0..utf8.len())
    }
    /// Produce a shared immutable byte-range lease, not a guest slicing recipe.
    /// String::as_str and mutable buffers need their actual owner/version joins.
    pub fn immutable_str_range_ticket(
        &mut self,
        utf8: &[u8],
        range: Range<usize>,
    ) -> Result<TicketId, Diagnostic> {
        let prepared = self.reserve_immutable_str_range(utf8, range, false)?;
        self.commit_immutable_str(prepared)
    }
    /// Immutable quota/source-data admission before fuel or arena mutation.
    pub(crate) fn preflight_immutable_str_literal(
        &self,
        utf8: &[u8],
        needs_cell: bool,
    ) -> Result<(), Diagnostic> {
        if utf8.len() > self.budget.max_bytes() {
            return Err(resource());
        }
        let text = std::str::from_utf8(utf8).map_err(|_| utf8_fault())?;
        let new = self.strings.find(Digest::of_bytes(utf8), text)?.is_err();
        let handles = 2u32
            .checked_add(u32::from(new))
            .and_then(|n| n.checked_add(u32::from(needs_cell)))
            .ok_or_else(resource)?;
        let cells = self
            .cells
            .atomic_reservation_bytes(usize::from(needs_cell))?;
        let refs = self.references.atomic_reservation_bytes(1)?;
        let tickets = self.tickets.atomic_reservation_bytes(1)?;
        let bytes = self
            .strings
            .reservation_bytes(new)?
            .checked_add(cells)
            .and_then(|n| n.checked_add(refs))
            .and_then(|n| n.checked_add(tickets))
            .and_then(|n| n.checked_add(if new { utf8.len() } else { 0 }))
            .ok_or_else(resource)?;
        self.budget.check(handles, bytes)
    }
    pub(crate) fn reserve_immutable_str_literal(
        &mut self,
        utf8: &[u8],
        needs_cell: bool,
    ) -> Result<PreparedImmutableStr, Diagnostic> {
        self.reserve_immutable_str_range(utf8, 0..utf8.len(), needs_cell)
    }
    fn reserve_immutable_str_range(
        &mut self,
        utf8: &[u8],
        range: Range<usize>,
        needs_cell: bool,
    ) -> Result<PreparedImmutableStr, Diagnostic> {
        // Bound incoming work before UTF-8 validation or hashing. Dedup reuse
        // does not charge bytes twice, but cannot exceed the run's byte limit.
        if utf8.len() > self.budget.max_bytes() {
            return Err(resource());
        }
        let text = std::str::from_utf8(utf8).map_err(|_| utf8_fault())?;
        text.get(range.clone()).ok_or_else(range_fault)?;
        let digest = Digest::of_bytes(utf8);
        let found = self.strings.find(digest, text)?;
        let new = found.is_err();
        let handles = 2u32
            .checked_add(u32::from(new))
            .and_then(|n| n.checked_add(u32::from(needs_cell)))
            .ok_or_else(resource)?;
        let tickets = self.tickets.atomic_reservation_bytes(1)?;
        let cells = self
            .cells
            .atomic_reservation_bytes(usize::from(needs_cell))?;
        let capacity = self
            .strings
            .reservation_bytes(new)?
            .checked_add(self.references.atomic_reservation_bytes(1)?)
            .and_then(|n| n.checked_add(tickets))
            .and_then(|n| n.checked_add(cells))
            .ok_or_else(resource)?;
        let text_bytes = if new { text.len() } else { 0 };
        self.budget.check(
            handles,
            capacity.checked_add(text_bytes).ok_or_else(resource)?,
        )?;
        let prepared = match found {
            Ok(id) => PreparedBacking::Existing(id),
            Err(position) => {
                let mut owned = String::new();
                owned
                    .try_reserve_exact(text.len())
                    .map_err(|_| resource())?;
                owned.push_str(text);
                // Account actual allocator capacity, not merely requested bytes.
                self.budget.check(
                    handles,
                    capacity
                        .checked_add(owned.capacity())
                        .ok_or_else(resource)?,
                )?;
                PreparedBacking::New {
                    digest,
                    position,
                    backing: ImmutableStrBacking { text: owned },
                }
            }
        };
        let owned = match &prepared {
            PreparedBacking::New { backing, .. } => backing.text.capacity(),
            _ => 0,
        };
        // This descriptor is the existing unsized Str target under a shared Ref;
        // it never permits a standalone value with descriptor16.
        let ty = self.types.intern(TypeKey::Ref {
            mutable: false,
            target: TypeId(16),
        })?;
        if needs_cell {
            self.cells.reserve_atomic(1, &mut self.budget)?;
        }
        self.strings.reserve(new, &mut self.budget)?;
        self.references.reserve_atomic(1, &mut self.budget)?;
        self.tickets.reserve_atomic(1, &mut self.budget)?;
        self.budget.check(handles, owned)?;

        Ok(PreparedImmutableStr {
            session: self.pattern_binding_session(),
            rows: self.strings.rows.len(),
            backing: prepared,
            ty,
            range,
            new,
            owned,
        })
    }
    pub(crate) fn commit_immutable_str(
        &mut self,
        prepared: PreparedImmutableStr,
    ) -> Result<TicketId, Diagnostic> {
        if prepared.session != self.pattern_binding_session()
            || prepared.rows != self.strings.rows.len()
        {
            return Err(ownership());
        }
        self.budget
            .check(2 + u32::from(prepared.new), prepared.owned)?;
        let backing = self.strings.commit(prepared.backing, &mut self.budget)?;
        let record = ReferenceRecord::from_immutable_buffer(BufferLease {
            backing,
            start: prepared.range.start,
            end: prepared.range.end,
        });
        let reference = self.references.allocate(record, 0, &mut self.budget)?;
        let value = ValueRepr {
            kind: 9,
            type_id: prepared.ty.0,
            payload: reference.bits(),
        };
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)?
            .into();
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
    }
    /// Resolve the actual current shared buffer lease. Ordinary locations,
    /// owned String tickets and forged descriptors are not byte views.
    pub fn str_reference_contents(&self, ticket: TicketId) -> Result<&str, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 9 {
            return Err(invalid());
        }
        let reference = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(reference)?;
        match &reference.lease {
            ReferenceLease::Buffer(buffer) => self.strings.resolve(*buffer),
            ReferenceLease::Location(_) => Err(invalid()),
        }
    }
}
#[cfg(test)]
#[path = "str_storage_tests.rs"]
mod tests;

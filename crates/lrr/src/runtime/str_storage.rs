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
    fn reserve(&mut self, new: bool, budget: &mut RunBudget) -> Result<(), Diagnostic> {
        if !new {
            return Ok(());
        }
        self.backings.reserve_atomic(1, budget)?;
        if self.rows.len() == self.rows.capacity() {
            // Build replacement capacity first so a failed allocation leaves the
            // live index buffer unchanged. Geometric growth stays amortized.
            let count = self.rows.len().checked_add(1).ok_or_else(resource)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count.max(self.rows.capacity().saturating_mul(2)))
                .map_err(|_| resource())?;
            let delta = rows
                .capacity()
                .checked_sub(self.rows.capacity())
                .and_then(|n| n.checked_mul(std::mem::size_of::<PoolRow>()))
                .ok_or_else(resource)?;
            rows.extend_from_slice(&self.rows);
            budget.charge(0, delta);
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
    /// Immutable source-data admission before arena mutation.
    pub(crate) fn preflight_immutable_str_literal(&self, utf8: &[u8]) -> Result<(), Diagnostic> {
        let text = std::str::from_utf8(utf8).map_err(|_| utf8_fault())?;
        self.strings.find(Digest::of_bytes(utf8), text).map(|_| ())
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
        let text = std::str::from_utf8(utf8).map_err(|_| utf8_fault())?;
        text.get(range.clone()).ok_or_else(range_fault)?;
        let digest = Digest::of_bytes(utf8);
        let found = self.strings.find(digest, text)?;
        let new = found.is_err();
        let prepared = match found {
            Ok(id) => PreparedBacking::Existing(id),
            Err(position) => {
                let mut owned = String::new();
                owned
                    .try_reserve_exact(text.len())
                    .map_err(|_| resource())?;
                owned.push_str(text);
                PreparedBacking::New {
                    digest,
                    position,
                    backing: ImmutableStrBacking { text: owned },
                }
            }
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

        Ok(PreparedImmutableStr {
            session: self.pattern_binding_session(),
            rows: self.strings.rows.len(),
            backing: prepared,
            ty,
            range,
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
            ReferenceLease::Location(_) | ReferenceLease::Elements(_) => Err(invalid()),
        }
    }
}
/// What a shared reference designates, for closed facade formatting.
pub(crate) enum ReferenceView<'a> {
    Str(&'a str),
    Value(ValueRepr),
}
impl GuestStore {
    /// Read-only view through an actual shared reference value: its str
    /// bytes, or the Copy value at its location. Aggregates are refused by
    /// the ordinary scalar read.
    /// The referent cell of a reference to a location (any value kind).
    pub(crate) fn referent_location_cell(
        &self,
        value: ValueRepr,
    ) -> Result<Option<super::CellId>, Diagnostic> {
        if value.kind != 9 {
            return Ok(None);
        }
        let reference = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(reference)?;
        Ok(match &reference.lease {
            ReferenceLease::Location(_) => Some(reference.location()?.final_cell.cell),
            _ => None,
        })
    }
    pub(crate) fn reference_view(&self, value: ValueRepr) -> Result<ReferenceView<'_>, Diagnostic> {
        if value.kind != 9 {
            return Err(invalid());
        }
        let reference = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(reference)?;
        if reference.access != Access::Shared {
            return Err(ownership());
        }
        match &reference.lease {
            ReferenceLease::Buffer(buffer) => Ok(ReferenceView::Str(self.strings.resolve(*buffer)?)),
            ReferenceLease::Location(_) => Ok(ReferenceView::Value(
                self.read_scalar(reference.location()?.final_cell.cell)?,
            )),
            ReferenceLease::Elements(_) => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "slice formatting requires its element-window formatting route".into(),
            )),
        }
    }
}
#[cfg(test)]
#[path = "str_storage_tests.rs"]
mod tests;

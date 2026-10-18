//! Memory replacement, interior mutability and leaked allocations.
//!
//! `mem::replace`/`swap` exchange the values of the locations two `&mut`
//! references designate; `Cell`/`RefCell`/`Rc` storage is replaced, taken and
//! re-leased mutably through a shared reference (rustc's `UnsafeCell`, whose
//! shared access rustc's own checks admit, never the interpreter's); a leaked
//! `Box` lives in a never-retired location so the shared references made to it
//! stay valid for the rest of the run.
use super::*;

impl GuestStore {
    /// The cell a reference to one sized location designates.
    fn reference_cell(&self, reference: ValueRepr, mutable: bool) -> Result<CellId, Diagnostic> {
        if reference.kind != 9 {
            return Err(invalid());
        }
        let record = self.references.get(RefId::from_bits(reference.payload))?;
        self.validate_reference_record(record)?;
        if mutable && record.access != Access::Mutable {
            return Err(access_fault());
        }
        Ok(record.location()?.final_cell.cell)
    }
    /// An unconstrained numeric literal takes the declared Type of the
    /// location it is stored in.
    fn bind_literal_for(&mut self, incoming: TicketId, cell: CellId) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(incoming)?;
        if matches!(value.kind, 11 | 12) && self.effective_type(value)?.is_none() {
            if let StorageContract::Exact(ty) = self.cells.get(cell)?.declared {
                self.materialize(incoming, ty)?;
            }
        }
        Ok(())
    }
    /// `mem::replace(dest, src)`: the old value of `*dest`.
    pub(crate) fn mem_replace(
        &mut self,
        reference: ValueRepr,
        incoming: TicketId,
    ) -> Result<TicketId, Diagnostic> {
        let cell = self.reference_cell(reference, true)?;
        self.bind_literal_for(incoming, cell)?;
        self.replace(cell, incoming)
    }
    /// `mem::swap(a, b)`.
    pub(crate) fn mem_swap(&mut self, a: ValueRepr, b: ValueRepr) -> Result<(), Diagnostic> {
        let (first, second) = (self.reference_cell(a, true)?, self.reference_cell(b, true)?);
        self.swap_cell_values(first, second)
    }
    /// `Cell::replace`: replace the value at a shared reference's location.
    pub(crate) fn cell_replace(
        &mut self,
        reference: ValueRepr,
        incoming: TicketId,
    ) -> Result<TicketId, Diagnostic> {
        let cell = self.reference_cell(reference, false)?;
        self.bind_literal_for(incoming, cell)?;
        self.replace_with(cell, incoming, false)
    }
    /// Move the value out of a shared reference's location (the location is
    /// left moved-out; `Rc` takes its contents out at its last release).
    pub(crate) fn cell_take(&mut self, reference: ValueRepr) -> Result<TicketId, Diagnostic> {
        let cell = self.reference_cell(reference, false)?;
        self.move_from(cell)
    }
    /// `&mut` access to the location a shared reference of interior-mutable
    /// storage designates (`UnsafeCell::get`).
    pub(crate) fn shared_to_mut(&mut self, reference: ValueRepr) -> Result<TicketId, Diagnostic> {
        if reference.kind != 9 {
            return Err(invalid());
        }
        let record = self.references.get(RefId::from_bits(reference.payload))?;
        self.validate_reference_record(record)?;
        let location = record.location()?;
        let mut enclosing = Vec::new();
        enclosing
            .try_reserve_exact(location.enclosing.len())
            .map_err(|_| resource())?;
        enclosing.extend_from_slice(&location.enclosing);
        let mut source_projections = Vec::new();
        source_projections
            .try_reserve_exact(location.source_projections.len())
            .map_err(|_| resource())?;
        source_projections.extend_from_slice(&location.source_projections);
        let target = record.target;
        let lease = crate::runtime::reference::LocationLease {
            root: location.root,
            enclosing,
            final_cell: location.final_cell,
            source_projections,
        };
        let mutable = ReferenceRecord {
            access: Access::Mutable,
            owner: None,
            interior: true,
            raw: false,
            target,
            lease: ReferenceLease::Location(lease),
        };
        let bytes = mutable.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        let id: RefId = self
            .references
            .allocate(mutable, bytes, &mut self.budget)?
            .into();
        self.reference_ticket(id)
    }
    /// Whether two references designate the same location.
    pub(crate) fn ref_eq(&self, a: ValueRepr, b: ValueRepr) -> Result<bool, Diagnostic> {
        let lease = |value: ValueRepr| -> Result<Option<CellId>, Diagnostic> {
            if value.kind != 9 {
                return Err(invalid());
            }
            let record = self.references.get(RefId::from_bits(value.payload))?;
            Ok(match &record.lease {
                ReferenceLease::Location(location) => Some(location.final_cell.cell),
                _ => None,
            })
        };
        Ok(match (lease(a)?, lease(b)?) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        })
    }
    /// A never-retired location owning `ticket`'s value for the rest of the
    /// run (the `Box::leak` allocation, a forgotten value).
    fn leak_value(&mut self, ticket: TicketId) -> Result<CellId, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        let ty = self.effective_type(value)?.ok_or_else(invalid)?;
        let anchor = match self.leak_anchor {
            Some(anchor) => anchor,
            None => {
                let anchor = self.new_frame_anchor()?;
                self.leak_anchor = Some(anchor);
                anchor
            }
        };
        let slot = self.leaked;
        self.leaked = slot.checked_add(1).ok_or_else(resource)?;
        self.cells.reserve(1, &mut self.budget)?;
        let cell = self.new_cell(
            StorageContract::Exact(ty),
            false,
            CellOwner::FrameSlot { frame: anchor, slot },
        )?;
        self.initialize(cell, ticket)?;
        Ok(cell)
    }
    /// `mem::forget`: the value moves into a never-retired location.
    pub(crate) fn forget_value(&mut self, ticket: TicketId) -> Result<TicketId, Diagnostic> {
        self.leak_value(ticket)?;
        self.ticket(ValueRepr::unit())
    }
    /// `Box::leak`: the box moves into a never-retired location and a shared
    /// reference to its contents is returned.
    pub(crate) fn leak_box(&mut self, boxed: TicketId) -> Result<TicketId, Diagnostic> {
        let value = self.peek_ticket(boxed)?;
        if value.kind != 8 || self.box_contents_type(TypeId(value.type_id))?.is_none() {
            return Err(invalid());
        }
        let cell = self.leak_value(boxed)?;
        let place = Place::cell(cell).projected(Projection::BoxContents)?;
        self.borrow_ticket(&place, Access::Shared)
            .map_err(|failure| match failure {
                PlaceFailure::RuntimeFault(error) => error,
                PlaceFailure::GuestPanic(_) => invalid(),
            })
    }
}

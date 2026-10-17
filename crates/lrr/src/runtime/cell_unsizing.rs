//! Built-in reference unsizing coercion and element-window projections.
//!
//! rustc's `CoerceUnsized` for built-in references: `&[T; N]` / `&mut [T; N]`
//! coerce to `&[T]` / `&mut [T]` (and `&mut [T; N]` to `&[T]`). This is the
//! one runtime operation every coercion site (call arguments, typed `let`
//! bindings, typed returns) applies: the coerced reference names the same
//! array location as its complete element window. Trait-object coercions do
//! not change the reference value (objects dispatch by the referent's
//! reached Type), so they never reach this operation.
use super::*;

/// The admitted unsizing of one reference value to an expected descriptor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct UnsizeCoercion {
    access: Access,
    slice: TypeId,
    len: u32,
}

impl GuestStore {
    /// Pure admission: `Some` exactly when `value` is a live reference to an
    /// array whose `[T]` reference view is `expected` (never adding `mut`).
    pub(crate) fn unsizing_coercion(
        &self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<Option<UnsizeCoercion>, Diagnostic> {
        // `Box<[T; N]>` -> `Box<[T]>` (see `cell_boxes.rs`).
        if value.kind == 8 {
            return Ok(self
                .box_unsizing(value, expected)?
                .map(|(slice, len)| UnsizeCoercion {
                    access: Access::Shared,
                    slice,
                    len,
                }));
        }
        if value.kind != 9 {
            return Ok(None);
        }
        let TypeKey::Ref {
            mutable: to_mutable,
            target: slice,
        } = self.types.descriptor(expected)?.key.as_ref()
        else {
            return Ok(None);
        };
        let TypeKey::Ref {
            mutable: from_mutable,
            target: array,
        } = self.types.descriptor(TypeId(value.type_id))?.key.as_ref()
        else {
            return Ok(None);
        };
        if *to_mutable && !*from_mutable {
            return Ok(None);
        }
        let (TypeKey::Slice { element }, TypeKey::Array { element: actual, len }) = (
            self.types.descriptor(*slice)?.key.as_ref(),
            self.types.descriptor(*array)?.key.as_ref(),
        ) else {
            return Ok(None);
        };
        if element != actual {
            return Ok(None);
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        record.location()?;
        Ok(Some(UnsizeCoercion {
            access: if *to_mutable {
                Access::Mutable
            } else {
                Access::Shared
            },
            slice: *slice,
            len: *len,
        }))
    }
    /// Consume an owned reference value and publish its coerced `[T]` view.
    /// A consumed mutable lease is destroyed; a shared lease may still be
    /// named by Copy duplicates and stays.
    fn coerce_unsized_value(
        &mut self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        let coercion = self
            .unsizing_coercion(value, expected)?
            .ok_or_else(ownership)?;
        if value.kind == 8 {
            return self.coerce_unsized_box(value, expected);
        }
        let id = RefId::from_bits(value.payload);
        let record = self.references.get(id)?;
        let consumed = record.access == Access::Mutable;
        let coerced = record.unsized_array(coercion.access, coercion.slice, coercion.len)?;
        let bytes = coerced.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        let reference: RefId = self
            .references
            .allocate(coerced, bytes, &mut self.budget)?
            .into();
        if consumed {
            self.references.take(id, &mut self.budget)?;
        }
        Ok(ValueRepr {
            kind: 9,
            type_id: expected.0,
            payload: reference.bits(),
        })
    }
    /// Coerce the reference a transfer ticket owns, in place.
    pub(crate) fn coerce_unsized_ticket(
        &mut self,
        ticket: TicketId,
        expected: TypeId,
    ) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        let coerced = self.coerce_unsized_value(value, expected)?;
        self.tickets.get_mut(ticket)?.value = coerced;
        self.set_owner(coerced, ValueOwner::Ticket(ticket))
    }
    /// Coerce the reference a live local owns, in place (a typed `let`
    /// binding whose annotation names the slice reference). The local's
    /// storage contract must not already fix a different Type.
    pub(crate) fn coerce_unsized_cell(
        &mut self,
        cell: CellId,
        expected: TypeId,
    ) -> Result<(), Diagnostic> {
        let value = self.admit_move(cell)?;
        // An unannotated local's contract was bound to its initializer's own
        // Type at initialization; the coercion rebinds it to the slice view.
        match self.cells.get(cell)?.declared {
            StorageContract::Unbound => {}
            StorageContract::Exact(ty) if ty == expected || ty.0 == value.type_id => {}
            _ => return Err(ownership()),
        }
        let coerced = self.coerce_unsized_value(value, expected)?;
        let record = self.cells.get_mut(cell)?;
        record.state = CellState::Live(coerced);
        if matches!(record.declared, StorageContract::Exact(_)) {
            record.declared = StorageContract::Exact(expected);
        }
        self.set_owner(coerced, ValueOwner::Cell(cell))
    }
    /// Coerce a value admitted from a cell before it is sealed elsewhere.
    pub(super) fn coerce_unsized_admitted(
        &mut self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        self.coerce_unsized_value(value, expected)
    }
    /// Whether `value` is a live `&mut` reference (its lease is unique).
    pub(crate) fn reference_is_mutable(&self, value: ValueRepr) -> Result<bool, Diagnostic> {
        if value.kind != 9 {
            return Ok(false);
        }
        Ok(self.references.get(RefId::from_bits(value.payload))?.access == Access::Mutable)
    }
    /// The element window a reference designates: `(start, len)` of its array
    /// location for a slice reference, `None` for a sized location.
    pub(crate) fn reference_window(&self, id: RefId) -> Result<Option<(u32, u32)>, Diagnostic> {
        let record = self.references.get(id)?;
        self.validate_reference_record(record)?;
        Ok(record
            .elements()
            .map(|elements| (elements.start, elements.len)))
    }
    /// `len()` of a `[T]` or `[T; N]` place reached through a reference value:
    /// the window length of a slice reference or the array's length.
    pub(crate) fn referent_len(&self, value: ValueRepr) -> Result<u64, Diagnostic> {
        if value.kind != 9 {
            return Err(invalid());
        }
        let id = RefId::from_bits(value.payload);
        let record = self.references.get(id)?;
        self.validate_reference_record(record)?;
        match &record.lease {
            ReferenceLease::Elements(elements) => Ok(u64::from(elements.len)),
            ReferenceLease::Buffer(buffer) => Ok(self.strings.resolve(*buffer)?.len() as u64),
            ReferenceLease::Location(_) => match self.types.descriptor(record.target)?.key.as_ref()
            {
                TypeKey::Array { len, .. } => Ok(u64::from(*len)),
                _ => Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "len requires a slice, array or str referent".into(),
                )),
            },
        }
    }
    /// A live `core::ops::Range*<usize>` value of the core facade crate
    /// (`core`): its rustc `SliceIndex` bounds. Any other value is `None`.
    /// `core` is demanded only for a nominal `ops::Range*`-shaped operand.
    pub(crate) fn range_operand(
        &self,
        cell: CellId,
        core: impl FnOnce() -> crate::core::digest::Digest,
    ) -> Result<Option<crate::runtime::place::SliceRange>, Diagnostic> {
        use crate::runtime::place::SliceRange;
        let CellState::Live(value) = self.cells.get(cell)?.state else {
            return Ok(None);
        };
        if value.kind != 8 {
            return Ok(None);
        }
        let TypeKey::Nominal { definition, .. } =
            self.types.descriptor(TypeId(value.type_id))?.key.as_ref()
        else {
            return Ok(None);
        };
        if definition.module_path.len() != 1
            || definition.module_path[0] != "ops"
            || !definition.declared_name.starts_with("Range")
            || definition.crate_key != core()
        {
            return Ok(None);
        }
        let (_, storage) = self.place_object(cell)?;
        let field = |index: usize| -> Result<u64, Diagnostic> {
            let child = *storage.fields.get(index).ok_or_else(invalid)?;
            let value = self.read_scalar(child)?;
            crate::runtime::place::checked_index(value)
        };
        // Declaration-order fields of the core facade's range structs.
        Ok(Some(match definition.declared_name.as_str() {
            "RangeFull" => SliceRange {
                start: 0,
                end: None,
                inclusive: false,
            },
            "Range" => SliceRange {
                start: field(0)?,
                end: Some(field(1)?),
                inclusive: false,
            },
            "RangeFrom" => SliceRange {
                start: field(0)?,
                end: None,
                inclusive: false,
            },
            "RangeTo" => SliceRange {
                start: 0,
                end: Some(field(0)?),
                inclusive: false,
            },
            "RangeInclusive" => SliceRange {
                start: field(0)?,
                end: Some(field(1)?),
                inclusive: true,
            },
            "RangeToInclusive" => SliceRange {
                start: 0,
                end: Some(field(0)?),
                inclusive: true,
            },
            _ => return Ok(None),
        }))
    }
    /// Whether `cell` holds a live aggregate value (cheap kind probe).
    pub(crate) fn holds_aggregate(&self, cell: CellId) -> Result<bool, Diagnostic> {
        Ok(matches!(self.cells.get(cell)?.state, CellState::Live(value) if value.kind == 8))
    }
    /// The element count of a `[T]` place (a slice reference's window).
    pub(crate) fn slice_place_len(&self, place: &Place) -> Result<u32, PlaceFailure> {
        let resolved = self.resolve_place_windowed(place, PlaceUse::Address)?;
        resolved
            .window
            .map(|(_, len)| len)
            .ok_or_else(|| PlaceFailure::RuntimeFault(invalid()))
    }
    /// `base[range]`: the checked element window of an array place or of a
    /// slice window, with rustc's `SliceIndex<[T]>` bounds panics.
    pub(crate) fn project_range(
        &self,
        base: &Place,
        range: crate::runtime::place::SliceRange,
    ) -> Result<Place, PlaceFailure> {
        let resolved = self.resolve_place_windowed(base, PlaceUse::Address)?;
        let available = match (resolved.window, self.types.descriptor(resolved.ty)?.key.as_ref()) {
            (Some((_, len)), _) => len,
            (None, TypeKey::Array { len, .. }) => *len,
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "range indexing requires an array or slice place".into(),
                )
                .into());
            }
        };
        let (start, end) = crate::runtime::place::checked_slice_range(range, u64::from(available))
            .map_err(PlaceFailure::GuestPanic)?;
        let place = base.projected(Projection::Range {
            start: u32::try_from(start).map_err(|_| resource())?,
            end: u32::try_from(end).map_err(|_| resource())?,
        })?;
        self.resolve_place_windowed(&place, PlaceUse::Address)?;
        Ok(place)
    }
    /// The element cells of the array value live in `cell` (a `[T; N]`
    /// Debug list's entries), or `None` when it holds no array.
    pub(crate) fn array_element_cells(&self, cell: CellId) -> Result<Option<Vec<CellId>>, Diagnostic> {
        let CellState::Live(value) = self.cells.get(cell)?.state else {
            return Ok(None);
        };
        if value.kind != 8
            || !matches!(
                self.types.descriptor(TypeId(value.type_id))?.key.as_ref(),
                TypeKey::Array { .. }
            )
        {
            return Ok(None);
        }
        let (_, storage) = self.place_object(cell)?;
        Ok(Some(storage.fields.to_vec()))
    }
    /// The element cells a reference to `[T; N]` or `[T]` designates (a
    /// slice reference's window): the entries of the referent's Debug list.
    /// `None` for any other value. Reading never moves or borrows.
    pub(crate) fn referent_element_cells(
        &self,
        value: ValueRepr,
    ) -> Result<Option<Vec<CellId>>, Diagnostic> {
        if value.kind != 9 {
            return Ok(None);
        }
        let id = RefId::from_bits(value.payload);
        let record = self.references.get(id)?;
        self.validate_reference_record(record)?;
        if !matches!(
            self.types.descriptor(record.target)?.key.as_ref(),
            TypeKey::Array { .. } | TypeKey::Slice { .. }
        ) {
            return Ok(None);
        }
        let resolved = self
            .resolve_place_windowed(&Place::deref(id), PlaceUse::Address)
            .map_err(|failure| match failure {
                PlaceFailure::RuntimeFault(error) => error,
                PlaceFailure::GuestPanic(_) => invalid(),
            })?;
        let (_, storage) = self.place_object(resolved.target.cell)?;
        Ok(Some(match resolved.window {
            Some((start, len)) => storage
                .fields
                .get(start as usize..(start as usize).checked_add(len as usize).ok_or_else(invalid)?)
                .ok_or_else(invalid)?
                .to_vec(),
            None => storage.fields.to_vec(),
        }))
    }
}

#[cfg(test)]
#[path = "cell_unsizing_tests.rs"]
mod tests;

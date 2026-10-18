//! Physical, non-consuming comparison of an evaluated constant's live storage.
//! Source selection and alternative/guard order belong to the single Search.
use super::*;
use std::collections::BTreeSet;

/// A retained initializer owner, never permission to duplicate its value.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ConstantRoot {
    Cell(CellId),
    Ticket(TicketId),
}

fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "constant pattern storage kind has no structural comparison".into(),
    )
}

impl GuestStore {
    pub(crate) fn pattern_constant_value(
        &self,
        root: ConstantRoot,
    ) -> Result<ValueRepr, Diagnostic> {
        match root {
            ConstantRoot::Cell(cell) => self.admit_move(cell),
            ConstantRoot::Ticket(ticket) => {
                let value = self.peek_ticket(ticket)?;
                self.object_owner(value, ValueOwner::Ticket(ticket))?;
                self.require_initialized_value(value)?;
                Ok(value)
            }
        }
    }

    fn pattern_field(
        &self,
        object: ObjectId,
        storage: &AggregateStorage,
        index: usize,
    ) -> Result<ValueRepr, Diagnostic> {
        let cell = *storage.fields.get(index).ok_or_else(ownership)?;
        let ordinal = u32::try_from(index).map_err(|_| ownership())?;
        if self.cells.get(cell)?.owner
            != (CellOwner::Field {
                object,
                index: ordinal,
            })
            || !storage.initialized.field(index)?
        {
            return Err(ownership());
        }
        self.admit_move(cell)
    }

    /// Actual source-selected values supply comparison data, not member
    /// access authority. No ownership, leases or guest callbacks are created.
    /// Scalar leaves use the matcher's existing numeric comparison.
    pub(crate) fn compare_pattern_constant(
        &self,
        actual: &Place,
        expected: ConstantRoot,
        scalar_equal: impl Fn(ValueRepr, ValueRepr) -> Result<bool, Diagnostic>,
    ) -> Result<bool, PlaceFailure> {
        let actual = self.resolve_place(actual)?;
        let mut work = vec![(
            self.admit_move(actual.target.cell)?,
            self.pattern_constant_value(expected)?,
        )];
        let mut visited = BTreeSet::new();
        while let Some((actual, expected)) = work.pop() {
            self.require_initialized_value(actual)?;
            self.require_initialized_value(expected)?;
            match (actual.kind, expected.kind) {
                (8, 8) => {
                    if actual.type_id != expected.type_id {
                        return Ok(false);
                    }
                    let a = ObjectId::from_bits(actual.payload);
                    let b = ObjectId::from_bits(expected.payload);
                    match (&self.objects.get(a)?.storage, &self.objects.get(b)?.storage) {
                        (
                            ObjectStorage::Aggregate(a_fields),
                            ObjectStorage::Aggregate(b_fields),
                        ) => {
                            if a_fields.next_cleanup != 0
                                || b_fields.next_cleanup != 0
                                || a_fields.pending_cleanup.is_some()
                                || b_fields.pending_cleanup.is_some()
                            {
                                return Err(ownership().into());
                            }
                            if a_fields.variant != b_fields.variant
                                || a_fields.fields.len() != b_fields.fields.len()
                            {
                                return Ok(false);
                            }
                            if !visited.insert((
                                8,
                                actual.type_id,
                                actual.payload,
                                expected.payload,
                            )) {
                                continue;
                            }
                            for index in (0..a_fields.fields.len()).rev() {
                                work.push((
                                    self.pattern_field(a, a_fields, index)?,
                                    self.pattern_field(b, b_fields, index)?,
                                ));
                            }
                        }
                        (ObjectStorage::String(a), ObjectStorage::String(b)) => {
                            if a != b {
                                return Ok(false);
                            }
                        }
                        _ => return Err(unsupported().into()),
                    }
                }
                (9, 9) => {
                    if actual.type_id != expected.type_id {
                        return Ok(false);
                    }
                    let a = self.references.get(RefId::from_bits(actual.payload))?;
                    let b = self.references.get(RefId::from_bits(expected.payload))?;
                    self.validate_reference_record(a)?;
                    self.validate_reference_record(b)?;
                    if a.raw || b.raw {
                        return Err(unsupported().into());
                    }
                    if !visited.insert((9, actual.type_id, actual.payload, expected.payload)) {
                        continue;
                    }
                    match (&a.lease, &b.lease) {
                        (ReferenceLease::Location(a), ReferenceLease::Location(b)) => {
                            work.push((
                                self.admit_move(a.final_cell.cell)?,
                                self.admit_move(b.final_cell.cell)?,
                            ));
                        }
                        (ReferenceLease::Buffer(a), ReferenceLease::Buffer(b)) => {
                            if self.strings.resolve(*a)? != self.strings.resolve(*b)? {
                                return Ok(false);
                            }
                        }
                        (ReferenceLease::Elements(a), ReferenceLease::Elements(b)) => {
                            if a.len != b.len {
                                return Ok(false);
                            }
                            let (a_object, a_fields) = self.place_object(a.base.final_cell.cell)?;
                            let (b_object, b_fields) = self.place_object(b.base.final_cell.cell)?;
                            for offset in (0..a.len).rev() {
                                let ai = usize::try_from(
                                    a.start.checked_add(offset).ok_or_else(ownership)?,
                                )
                                .map_err(|_| ownership())?;
                                let bi = usize::try_from(
                                    b.start.checked_add(offset).ok_or_else(ownership)?,
                                )
                                .map_err(|_| ownership())?;
                                work.push((
                                    self.pattern_field(a_object, a_fields, ai)?,
                                    self.pattern_field(b_object, b_fields, bi)?,
                                ));
                            }
                        }
                        _ => return Err(unsupported().into()),
                    }
                }
                (1, 1) => {}
                (8 | 9, _) | (_, 8 | 9) => return Ok(false),
                _ if !scalar_equal(actual, expected)? => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }
}

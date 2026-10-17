//! Closure call frames: the cells their captured variables are bound to.
//!
//! A by-value capture lives in its field of the closure value; a by-reference
//! capture's field is the borrow, and the closure body names its referent.
//! The closure frame binds each captured binding to that cell, so the body's
//! ordinary local reads, places and borrows reach the captured variable.
use super::*;

fn fault(failure: PlaceFailure) -> Diagnostic {
    match failure {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => invalid(),
    }
}

impl GuestStore {
    pub(crate) fn closure_capture_cell(
        &self,
        closure: &Place,
        field: u32,
    ) -> Result<CellId, Diagnostic> {
        let ty = self.place_type_id(closure).map_err(fault)?;
        let borrowed = match self.types.descriptor(ty)?.key.as_ref() {
            TypeKey::Callable { borrowed, .. } => {
                *borrowed.get(field as usize).ok_or_else(invalid)?
            }
            _ => return Err(invalid()),
        };
        // Closure fields are not guest-addressable; only the closure call
        // projects them, as structural tuple fields.
        let place = closure.projected(Projection::Tuple(field))?;
        let cell = self.resolve_place(&place).map_err(fault)?.target.cell;
        if !borrowed {
            return Ok(cell);
        }
        let reference = self.references.get(self.reference_from_cell(cell)?)?;
        let location = reference.location()?;
        if !location.enclosing.is_empty() || !location.source_projections.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "closure capture of a projected place awaits precise capture paths".into(),
            ));
        }
        Ok(location.final_cell.cell)
    }
}

impl GuestStore {
    /// Copy a Copy aggregate out of `cell` completely (a by-value capture of
    /// a Copy tuple, array or closure value), through the same bounded
    /// aggregate-copy machine an owning read uses.
    pub(crate) fn copy_aggregate_value(&mut self, cell: CellId) -> Result<TicketId, Diagnostic> {
        let mut copy = aggregate_copy::AggregateCopy::begin(self, cell)?;
        loop {
            if let aggregate_copy::CopyProgress::Complete(ticket) = copy.advance(self)? {
                return Ok(ticket);
            }
        }
    }
}

//! Scalar atomic operations in the serialized GuestStore memory domain.
//! No guest execution, callback or yield occurs within this operation. The
//! exclusive store borrow is its linearization boundary, including replacement;
//! every valid Ordering consequently observes the stronger SeqCst program order.
//! Guest-thread scheduling must preserve this shared-memory serialization.
use super::*;
use crate::runtime::shape::{Primitive, primitive};

impl GuestStore {
    pub(crate) fn atomic_op(
        &mut self,
        reference: ValueRepr,
        incoming: ValueRepr,
        operation: u64,
        ordering: u64,
    ) -> Result<Result<TicketId, &'static str>, Diagnostic> {
        if ordering > 4 || operation > 8 {
            return Err(invalid());
        }
        match (operation, ordering) {
            (0, 1) => return Ok(Err("there is no such thing as a release load")),
            (0, 3) => return Ok(Err("there is no such thing as an acquire-release load")),
            (1, 2) => return Ok(Err("there is no such thing as an acquire store")),
            (1, 3) => return Ok(Err("there is no such thing as an acquire-release store")),
            _ => {}
        }
        let cell = self.reference_cell(reference, false)?;
        let old = self.admit_copy(cell)?;
        let ty = TypeId(old.type_id);
        let (signed, bits) = match primitive(ty)? {
            Primitive::Bool => (false, 1u8),
            Primitive::Signed(bits) => (true, bits),
            Primitive::Unsigned(bits) => (false, bits),
            _ => return Err(invalid()),
        };
        if operation == 0 {
            return self.ticket(old).map(Ok);
        }
        if ty.0 == 2 && matches!(operation, 3 | 4) {
            return Err(invalid());
        }
        if incoming.type_id != old.type_id {
            return Err(invalid());
        }
        incoming.validate_scalar()?;
        let mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let next = match operation {
            1 | 2 => incoming.payload,
            3 => old.payload.wrapping_add(incoming.payload),
            4 => old.payload.wrapping_sub(incoming.payload),
            5 => old.payload & incoming.payload,
            6 => old.payload | incoming.payload,
            7 => old.payload ^ incoming.payload,
            8 => !(old.payload & incoming.payload),
            _ => return Err(invalid()),
        } & mask;
        let value = if ty.0 == 2 {
            ValueRepr::boolean(next != 0)
        } else if signed {
            let shift = 64 - bits;
            ValueRepr::signed(ty, ((next << shift) as i64) >> shift)?
        } else {
            ValueRepr::unsigned(ty, next)?
        };
        // Admission and replacement are synchronous; no guest operation can
        // observe the interval between this read and the committed write.
        let replacement = self.ticket(value)?;
        self.replace_with(cell, replacement, false).map(Ok)
    }
}

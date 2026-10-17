//! `[element; count]`: the element is evaluated once; the array holds `count`
//! copies of it (Copy element), the element itself (`count == 1`), or nothing
//! (`count == 0`, the element is dropped).
use super::{Frame, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::SlotId,
    runtime::{
        cell::{CellState, GuestStore},
        shape::{TypeId, TypeKey},
        value::ValueRepr,
    },
};

impl Frame {
    fn repeat_operands(
        &self,
        source: SlotId,
        count: SlotId,
        store: &GuestStore,
    ) -> Result<(ValueRepr, u32), Diagnostic> {
        let cell = self.cell(source)?;
        let CellState::Live(element) = store.state(cell)? else {
            return Err(invalid("repeat element not live"));
        };
        // An unconstrained numeric literal element takes rustc's i32/f64
        // fallback (a bound literal keeps its bound type).
        let element = if matches!(element.kind, 11 | 12) {
            store.read_format_scalar(cell)?
        } else {
            element
        };
        let CellState::Live(length) = store.state(self.cell(count)?)? else {
            return Err(invalid("repeat count not live"));
        };
        // The count is a `usize` constant.
        if length.type_id != 13 || length.kind != 5 {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "repeat count requires a usize value".into(),
            ));
        }
        let length = u32::try_from(length.payload).map_err(|_| {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "repeat count exceeds the guest array length representation".into(),
            )
        })?;
        Ok((element, length))
    }
    /// Whether one element value may be duplicated by value (scalars and
    /// shared references); aggregate Copy goes through its resumable copy.
    fn repeat_copies(element: &ValueRepr, store: &GuestStore) -> Result<bool, Diagnostic> {
        Ok((1..=7).contains(&element.kind)
            || (element.kind == 9
                && matches!(
                    store.type_descriptor(TypeId(element.type_id))?.key.as_ref(),
                    TypeKey::Ref { mutable: false, .. }
                )))
    }
    pub(crate) fn preflight_array_repeat(
        &self,
        source: SlotId,
        count: SlotId,
        _destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let (element, length) = self.repeat_operands(source, count, store)?;
        if length > 1 && !Self::repeat_copies(&element, store)? {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "repeating a non-scalar element requires its aggregate Copy".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn array_repeat(
        &mut self,
        source: SlotId,
        count: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_array_repeat(source, count, destination, store)?;
        let literal = matches!(
            store.state(self.cell(source)?)?,
            CellState::Live(value) if matches!(value.kind, 11 | 12)
        );
        let (element, length) = self.repeat_operands(source, count, store)?;
        let element_type = TypeId(element.type_id);
        let array = store.intern_type(TypeKey::Array {
            element: element_type,
            len: length,
        })?;
        let moved = store.move_from(self.cell(source)?)?;
        let guard = store.begin_construction(&vec![element_type; length as usize])?;
        match length {
            0 => store.discard(moved)?,
            1 if !literal => store.construction_push(guard, 0, moved)?,
            _ => {
                for field in 0..length {
                    let copy = store.ticket(element)?;
                    store.construction_push(guard, field, copy)?;
                }
                store.discard(moved)?;
            }
        }
        let ticket = store.commit_construction(guard, array)?;
        let element_view = self.source_view(source)?;
        self.write(destination, ticket, store)?;
        if let Some(element) = element_view {
            self.set_source_view(
                destination,
                Some(crate::traits::term::TyTerm::Array(
                    Box::new(element),
                    crate::traits::term::ConstTerm::Value(u64::from(length)),
                )),
            )?;
        }
        Ok(())
    }
}

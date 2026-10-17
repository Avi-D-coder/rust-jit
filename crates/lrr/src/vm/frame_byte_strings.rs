//! Byte string literals. `b"ab"` is a `&'static [u8; 2]`: the bytes live in
//! the run's promoted storage for this site (built once), and each
//! evaluation writes a fresh shared reference to them.
use super::*;
use crate::traits::term::{ConstTerm, RegionTerm, TyTerm};

impl Frame {
    pub(crate) fn publish_byte_string(
        &mut self,
        bytes: &[u8],
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let u8_ty = crate::runtime::shape::PrimitiveIdentity::from_recipe(
            crate::ir::op::PrimitiveType::U8,
        )?
        .id();
        let len = u32::try_from(bytes.len())
            .map_err(|_| invalid("byte string length exceeds an array length"))?;
        let array_ty = store.intern_type(crate::runtime::shape::TypeKey::Array {
            element: u8_ty,
            len,
        })?;
        let site = self.pc.fingerprint();
        let cell = match store.promoted_cell(site, array_ty) {
            Some(cell) => cell,
            None => {
                let elements = vec![u8_ty; bytes.len()];
                let guard = store.begin_construction(&elements)?;
                for (ordinal, byte) in bytes.iter().enumerate() {
                    let ticket = store.ticket(crate::runtime::value::ValueRepr::unsigned(
                        u8_ty,
                        u64::from(*byte),
                    )?)?;
                    store.construction_push(guard, ordinal as u32, ticket)?;
                }
                let ticket = store.commit_construction(guard, array_ty)?;
                store.install_promoted(site, array_ty, ticket)?
            }
        };
        let fault = |failure: crate::runtime::place::PlaceFailure| match failure {
            crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
            crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                invalid("promoted byte string place reported a bounds panic")
            }
        };
        // An unsizing expectation (`let s: &[u8] = b"ab";`, a `&[u8]`
        // result) borrows the array's complete element window, as a borrow
        // of an array place does.
        let unsized_window = self
            .borrow_unsizing(destination, array_ty, false, store)?
            .is_some();
        let place = if unsized_window {
            store
                .project_range(
                    &crate::runtime::place::Place::cell(cell),
                    crate::runtime::place::SliceRange {
                        start: 0,
                        end: None,
                        inclusive: false,
                    },
                )
                .map_err(fault)?
        } else {
            crate::runtime::place::Place::cell(cell)
        };
        store
            .reserve_place_borrow_result(&place, self.slot(destination)?.cell.is_none())
            .map_err(fault)?;
        self.prepare_write(destination, store)?;
        let ticket = store
            .borrow_ticket(&place, crate::runtime::reference::Access::Shared)
            .map_err(fault)?;
        self.write(destination, ticket, store)?;
        let target = if unsized_window {
            TyTerm::Slice(Box::new(TyTerm::Primitive(crate::ir::op::PrimitiveType::U8)))
        } else {
            TyTerm::Array(
                Box::new(TyTerm::Primitive(crate::ir::op::PrimitiveType::U8)),
                ConstTerm::Value(u64::from(len)),
            )
        };
        self.set_source_view(
            destination,
            Some(TyTerm::Ref(RegionTerm::Static, false, Box::new(target))),
        )
    }
}

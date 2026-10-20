//! Slice element windows reached through the ordinary index operation.
//!
//! `base[range]` for an array or slice place is rustc's
//! `Index<Range*<usize>>` through `SliceIndex<[T]>`. Until the core facade's
//! `Index` impls are selectable this is the explicitly specified runtime
//! intrinsic selected at the ordinary `Op::ProjectIndex` when the evaluated
//! index operand is a `core::ops::Range*` value: the projection is the
//! checked element window, typed `[T]` (see `GuestStore::project_range`).
use super::Frame;
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::SlotId,
    runtime::{
        cell::{GuestStore, StorageContract},
        place::SliceRange,
        shape::{TypeId, TypeKey},
    },
    traits::term::TyTerm,
};

impl Frame {
    /// The evaluated index operand as a `core::ops::Range*` value, if it is
    /// one. A scalar index (the usize element route) is `None`.
    pub(crate) fn range_index(
        &self,
        index: SlotId,
        store: &GuestStore,
    ) -> Result<Option<SliceRange>, Diagnostic> {
        let Some(cell) = self.slot(index)?.cell else {
            return Ok(None);
        };
        // A scalar index (the usize element route) never names a range:
        // only a `ops::Range*`-shaped aggregate pays for the core identity.
        if !store.holds_aggregate(cell)? {
            return Ok(None);
        }
        store.range_operand(cell, || crate::facade::unit(self.body.profile(), "core"))
    }
    /// The `&[T]`/`&mut [T]` descriptor a borrow of an array place produces
    /// when its result is checked against a slice-reference expectation (a
    /// typed `let`, argument or field coercion site propagated into `&a`).
    /// The borrow then takes the array's complete element window: the same
    /// unsizing coercion `GuestStore::coerce_unsized_ticket` applies to an
    /// already formed reference.
    pub(crate) fn borrow_unsizing(
        &self,
        destination: SlotId,
        target: TypeId,
        mutable: bool,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        // An array, or a vector (rustc's deref coercion `&Vec<T>` to `&[T]`).
        let element = match store.type_descriptor(target)?.key.as_ref() {
            TypeKey::Array { element, .. } => *element,
            _ => match store.vec_element_type(target)? {
                Some(element) => element,
                None => return Ok(None),
            },
        };
        let Some(slice) = store.find_type(&TypeKey::Slice { element }) else {
            return Ok(None);
        };
        let Some(reference) = store.find_type(&TypeKey::Ref {
            mutable,
            target: slice,
        }) else {
            return Ok(None);
        };
        let result = self.slot(destination)?;
        if let Some(source) = result.source.as_ref() {
            return Ok(match source {
                TyTerm::Ref(_, expected, pointee)
                    if *expected == mutable && matches!(pointee.as_ref(), TyTerm::Slice(_)) =>
                {
                    self.require_borrow_source_target(Some(pointee), slice, store)?;
                    Some(reference)
                }
                _ => None,
            });
        }
        let expected = match result.cell {
            Some(cell) => match store.declared(cell)? {
                StorageContract::Exact(ty) => Some(ty),
                _ => None,
            },
            None if result.strict_hint => result.hint,
            None => None,
        };
        Ok((expected == Some(reference)).then_some(reference))
    }
    /// The `&[T]`/`&mut [T]` Type a typed `let` initializer unsizes to: its
    /// value is a reference to an array whose own Type the inexact written
    /// annotation (`&[T]`, `&mut [T]`, `&[_]`) does not admit, but whose slice
    /// reference view it does. `None` when no coercion applies.
    pub(crate) fn written_unsizing(
        &self,
        source: SlotId,
        written: &crate::ir::written_type::WrittenType,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let value = store.admit_move(self.cell(source)?)?;
        // `Box<[T; N]>` meets a written `Box<[T]>` by unsizing (see
        // runtime/cell_boxes.rs).
        if value.kind == 8 {
            let actual = TypeId(value.type_id);
            if self.admits_written_hint(written, actual, store)? {
                return Ok(None);
            }
            return Ok(match store.unsized_box_view(actual)? {
                Some(view) if self.admits_written_hint(written, view, store)? => Some(view),
                _ => None,
            });
        }
        if value.kind != 9 {
            return Ok(None);
        }
        let actual = TypeId(value.type_id);
        if self.admits_written_hint(written, actual, store)? {
            return Ok(None);
        }
        let TypeKey::Ref { mutable, target } = store.type_descriptor(actual)?.key.as_ref() else {
            return Ok(None);
        };
        let element = match store.type_descriptor(*target)?.key.as_ref() {
            TypeKey::Array { element, .. } => *element,
            _ => match store.vec_element_type(*target)? {
                Some(element) => element,
                None => return Ok(None),
            },
        };
        let Some(slice) = store.find_type(&TypeKey::Slice { element }) else {
            return Ok(None);
        };
        // `&mut [T; N]` reaches `&mut [T]` or `&[T]`; `&[T; N]` only `&[T]`.
        for to_mutable in [*mutable, false] {
            if let Some(reference) = store.find_type(&TypeKey::Ref {
                mutable: to_mutable,
                target: slice,
            }) && self.admits_written_hint(written, reference, store)?
            {
                return Ok(Some(reference));
            }
        }
        Ok(None)
    }
    /// Apply the admitted unsizing coercion to a just-bound local's value.
    pub(crate) fn coerce_binding(
        &mut self,
        binding: crate::ir::pc::NodeId,
        expected: TypeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self
            .bindings
            .get(binding.0 as usize)
            .copied()
            .flatten()
            .ok_or_else(|| super::invalid("coerced binding cell absent"))?;
        store.coerce_unsized_cell(cell, expected)
    }
    /// The slice-reference Type a value written to `id` unsizes to: the
    /// destination is a coercion site (a declared body result, a typed
    /// storage contract or a strict expectation) expecting `&[T]` while the
    /// value is a `&[T; N]`. `None` when the value already has that Type.
    pub(crate) fn write_unsizing(
        &self,
        id: SlotId,
        value: crate::runtime::value::ValueRepr,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        // A reference, or a box (`Box<[T; N]>` to `Box<[T]>`).
        if value.kind != 9 && value.kind != 8 {
            return Ok(None);
        }
        let slot = self.slot(id)?;
        let contract = match slot.cell {
            Some(cell) => match store.declared(cell)? {
                StorageContract::Exact(ty) => Some(ty),
                _ => None,
            },
            None => None,
        };
        let result = if Some(id) == self.body.nodes()[0].value {
            self.hint(id)?
        } else {
            None
        };
        let strict = if slot.strict_hint { slot.hint } else { None };
        for expected in [result, contract, strict].into_iter().flatten() {
            if expected.0 != value.type_id
                && store.unsizing_coercion(value, expected)?.is_some()
            {
                return Ok(Some(expected));
            }
        }
        Ok(None)
    }
}

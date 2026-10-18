//! Pointer casts and fn-pointer expectations, frame side (E-R5-008).
//!
//! `Op::PointerCast` converts an evaluated operand (a reference, a raw or fn
//! pointer, a function item or closure value, or an integer) to its written
//! pointer type through `GuestStore::pointer_cast`, the one operation every
//! pointer coercion and cast shares. Implicit coercions at typed locations
//! (`let p: *const T = &x`, a pointer parameter, a fn-pointer field) reach the
//! same operation through `GuestStore::unsizing_coercion`.
use super::*;
use crate::{
    ir::written_type::WrittenType,
    runtime::{cell::StorageContract, shape::TypeKey},
};

fn inference_hole() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "pointer cast target with an inference hole beyond `*const _` requires inference".into(),
    )
}

impl Frame {
    /// `source as <pointer type>` into `destination`.
    pub(crate) fn pointer_cast(
        &mut self,
        source: SlotId,
        target: &WrittenType,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let value = store.admit_move(self.cell(source)?)?;
        let descriptor = match target.materialize(store)? {
            Some(descriptor) => descriptor,
            None => Self::inferred_pointer(target, value, store)?,
        };
        let result = store.pointer_cast(value, descriptor)?;
        let ticket = store.ticket(result)?;
        self.write(destination, ticket, store)?;
        let view = store
            .descriptor_source_term(descriptor)
            .filter(Self::supported_source);
        self.set_source_view(destination, view)
    }
    /// `*const _`: the pointee of the operand reference or pointer.
    fn inferred_pointer(
        target: &WrittenType,
        value: ValueRepr,
        store: &mut GuestStore,
    ) -> Result<TypeId, Diagnostic> {
        let WrittenType::Ptr {
            mutable,
            target: pointee,
        } = target
        else {
            return Err(inference_hole());
        };
        // A trait-object pointee keeps its value's concrete type, as for
        // references, so `*const dyn Trait` casts like `*const _`.
        if !matches!(pointee.as_ref(), WrittenType::Infer | WrittenType::Dyn(_)) || value.kind != 9 {
            return Err(inference_hole());
        }
        let pointee = match store.type_descriptor(TypeId(value.type_id))?.key.as_ref() {
            TypeKey::Ref { target, .. } | TypeKey::RawPtr { target, .. } => *target,
            _ => return Err(inference_hole()),
        };
        store.raw_pointer_type(*mutable, pointee)
    }
    /// The fn-pointer descriptor `destination` expects, if any: a function
    /// item written there meets the expectation by pointer coercion.
    pub(crate) fn function_pointer_expectation(
        &self,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let slot = self.slot(destination)?;
        let mut expected = Vec::new();
        if let Some(cell) = slot.cell
            && let StorageContract::Exact(ty) = store.declared(cell)?
        {
            expected.push(ty);
        }
        if slot.strict_hint
            && let Some(ty) = slot.hint
        {
            expected.push(ty);
        }
        for ty in expected {
            if matches!(store.type_descriptor(ty)?.key.as_ref(), TypeKey::Function { .. }) {
                return Ok(Some(ty));
            }
        }
        Ok(None)
    }
}

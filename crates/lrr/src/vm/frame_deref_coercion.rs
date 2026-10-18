//! Deref coercion at reference coercion sites, frame side.
//!
//! rustc coerces `&T` to `&U` wherever a reference is expected -- call
//! arguments, `let` annotations, struct fields, returns -- when `T: Deref`
//! reaches `U` (`&Vec<T>` as `&[T]`, `&String` as `&str`, `&Rc<T>` as `&T`,
//! a user smart pointer as its target, transitively). `box_deref_coercion`
//! (builtin box dereference) is the only built-in step; every overloaded step
//! is a call of `Deref::deref` (`DerefMut::deref_mut` for `&mut`) through the
//! ordinary trait-call route, the operator activation of
//! `frame_operator_calls.rs`:
//!
//! - a **borrow** (`&x`) whose result expects a reference to another Type
//!   than the nominal value it borrows steps that value's place, then borrows
//!   the referent (`borrow_coercion_goal`);
//! - a **reference read** (`s` of type `&String` read into a `&str` slot) is
//!   the same step applied to the referent of the reference
//!   (`value_coercion_goal`): the returned reference is the coerced value.
//!
//! The expectation is the one `box_deref_coercion` reads: the destination's
//! retained source view, typed storage contract, or a call argument's
//! parameter hint. A coercion that needs no step -- an exact match, an open
//! expectation, a non-nominal or box value -- costs a comparison only.
use super::*;
use crate::{
    runtime::{cell::StorageContract, place::Place, shape::TypeKey},
    traits::term::TyTerm,
};

/// What a reference-typed destination expects of the pointee.
struct ExpectedPointee {
    /// `&mut` (`true`) or `&` of the expected reference, when stated.
    mutable: Option<bool>,
    /// The pointee's descriptor, when the contract or hint names one.
    descriptor: Option<TypeId>,
    /// The pointee's retained source view.
    source: Option<TyTerm>,
}

/// A borrow's deref coercion goal: the borrowed nominal Type and the closed
/// Type its result expects a reference to.
pub(crate) struct BorrowCoercionGoal {
    pub(crate) place_type: TypeId,
    pub(crate) expected: TyTerm,
}

/// Where a reference value is read from.
pub(crate) enum ValueRead {
    Local(crate::ir::pc::NodeId),
    Parameter(u32),
    Place(SlotId),
}

/// A reference read's deref coercion goal: the referent place the reference
/// designates, the referent's nominal Type, the access the destination
/// expects and the closed Type it expects the pointee to be.
pub(crate) struct ValueCoercionGoal {
    pub(crate) referent: Place,
    pub(crate) referent_type: TypeId,
    pub(crate) mutable: bool,
    pub(crate) expected: TyTerm,
}

impl Frame {
    fn expected_pointee(&self, destination: SlotId, store: &GuestStore) -> Option<ExpectedPointee> {
        let result = self.slot(destination).ok()?;
        if result.source.is_none() && result.hint.is_none() && result.cell.is_none() {
            return None;
        }
        let (mut mutable, source) = match result.source.as_ref() {
            Some(TyTerm::Ref(_, mutable, pointee)) => (Some(*mutable), Some(pointee.as_ref().clone())),
            _ => (None, None),
        };
        let contract = match result.cell {
            Some(cell) => match store.declared(cell) {
                Ok(StorageContract::Exact(ty)) => Some(ty),
                _ => None,
            },
            None => result.hint,
        };
        let mut descriptor = None;
        if let Some(descriptor_key) = contract.and_then(|ty| store.type_descriptor(ty).ok()) {
            if let TypeKey::Ref { mutable: hinted, target } = descriptor_key.key.as_ref() {
                descriptor = Some(*target);
                mutable = mutable.or(Some(*hinted));
            }
        }
        if source.is_none() && descriptor.is_none() {
            return None;
        }
        Some(ExpectedPointee {
            mutable,
            descriptor,
            source,
        })
    }
    /// The closed Type a reference to the nominal `ty` is expected to become,
    /// or `None` when no deref step is called for: an exact match, an open
    /// expectation (a parameter or trait object pointee) or an expectation
    /// the Type cannot be checked against.
    fn coercion_target(
        &self,
        ty: TypeId,
        expected: ExpectedPointee,
        store: &GuestStore,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        if expected.descriptor == Some(ty)
            || expected
                .descriptor
                .is_some_and(|expected| store.dyn_admits(expected, ty))
        {
            return Ok(None);
        }
        if let Some(pointee) = expected.source.as_ref() {
            if !crate::frontend::call_contracts::closed_term(pointee) {
                return Ok(None);
            }
            if self.require_borrow_source_target(Some(pointee), ty, store).is_ok() {
                return Ok(None);
            }
        }
        Ok(match expected.source {
            Some(pointee) => Some(pointee),
            None => expected
                .descriptor
                .and_then(|ty| store.descriptor_source_term(ty))
                .filter(crate::frontend::call_contracts::closed_term),
        })
    }
    fn is_overloaded_nominal(ty: TypeId, store: &GuestStore) -> Result<bool, Diagnostic> {
        Ok(matches!(store.type_descriptor(ty)?.key.as_ref(), TypeKey::Nominal { .. })
            && store.box_contents_type(ty)?.is_none())
    }
    /// A borrow whose result expects a reference to another Type than the
    /// nominal value it borrows (`&v` of a `Vec` expected as `&[T]`): the
    /// place's Type and the Type expected. A place not yet executed, a box
    /// (builtin) and an open expectation need no step.
    pub(crate) fn borrow_coercion_goal(
        &self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<BorrowCoercionGoal>, Diagnostic> {
        if !(self.body.slot_kind(place).is_ok_and(|kind| kind == SlotKind::Place)
            && self.slots.get(place.0 as usize).is_some_and(|slot| {
                slot.place.is_some() && self.active_scopes.contains(&slot.scope)
            }))
        {
            return Ok(None);
        }
        let Ok(current) = self.executed_place(place) else {
            return Ok(None);
        };
        let Ok(ty) = store.place_type_id(current) else {
            return Ok(None);
        };
        if !Self::is_overloaded_nominal(ty, store)? {
            return Ok(None);
        }
        let Some(expected) = self.expected_pointee(destination, store) else {
            return Ok(None);
        };
        Ok(self.coercion_target(ty, expected, store)?.map(|expected| BorrowCoercionGoal {
            place_type: ty,
            expected,
        }))
    }
    /// A read of a reference value (`s: &String` read into a `&str` slot):
    /// the referent place and Type when the destination expects a reference
    /// to another closed Type that `Deref` reaches.
    pub(crate) fn value_coercion_goal(
        &self,
        read: ValueRead,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<ValueCoercionGoal>, Diagnostic> {
        // Most reads have no reference expectation at all.
        let Some(expected) = self.expected_pointee(destination, store) else {
            return Ok(None);
        };
        let Some(expected_mutable) = expected.mutable else {
            return Ok(None);
        };
        let reference = match read {
            ValueRead::Local(binding) => {
                match self.bindings.get(binding.0 as usize).copied().flatten() {
                    Some(cell) => Place::cell(cell),
                    None => return Ok(None),
                }
            }
            ValueRead::Parameter(ordinal) => match self.parameter(ordinal) {
                Ok(cell) => Place::cell(cell),
                Err(_) => return Ok(None),
            },
            ValueRead::Place(slot) => match self.executed_place(slot) {
                Ok(place) => Place {
                    root: place.root,
                    projections: place.projections.clone(),
                },
                Err(_) => return Ok(None),
            },
        };
        let Ok(reference_type) = store.place_type_id(&reference) else {
            return Ok(None);
        };
        let TypeKey::Ref {
            mutable: value_mutable,
            target,
        } = store.type_descriptor(reference_type)?.key.as_ref()
        else {
            return Ok(None);
        };
        let (value_mutable, target) = (*value_mutable, *target);
        if !Self::is_overloaded_nominal(target, store)? || (expected_mutable && !value_mutable) {
            return Ok(None);
        }
        let Some(expected_term) = self.coercion_target(target, expected, store)? else {
            return Ok(None);
        };
        let Ok(referent) = store.dereference_place(&reference) else {
            return Ok(None);
        };
        Ok(Some(ValueCoercionGoal {
            referent,
            referent_type: target,
            mutable: expected_mutable,
            expected: expected_term,
        }))
    }
}

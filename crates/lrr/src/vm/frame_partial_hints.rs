//! Partially written Type context: a known structure with open parts, such
//! as `let t: (_, u8) = ..`, `let s: &[u8] = &[..]` (a slice reached through
//! unsizing), a generic field `(T, u8)` before its type arguments are fixed,
//! or a declared call parameter `Result<T, u32>` before `T` is. It names no
//! single descriptor, so it is carried to the structural or nominal
//! constructor (or borrow) that builds the value, whose members with an
//! exactly known Type take that Type as their ordinary hint.
use super::{Frame, SlotId, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::written_type::WrittenType,
    runtime::{cell::GuestStore, shape::TypeId},
};
use std::{collections::BTreeMap, sync::Arc};

/// Where a partial hint's declaration parameters (`WrittenType::Parameter`)
/// take their type arguments from.
#[derive(Clone)]
pub(crate) enum PartialArguments {
    /// A generic field of a pending nominal construction, by that
    /// construction's frame guard: the type arguments its completed fields
    /// fixed so far.
    Construction(SlotId),
    /// A call's declared parameter Type: the callee type arguments fixed by
    /// explicit arguments or earlier arguments, indexed by parameter ordinal.
    Fixed(Arc<[Option<TypeId>]>),
}
/// A partial written hint and the source of its parameters' arguments.
#[derive(Clone)]
pub(crate) struct PartialHint {
    written: Arc<WrittenType>,
    arguments: Option<PartialArguments>,
}
#[derive(Default)]
pub(crate) struct PartialHints(BTreeMap<SlotId, PartialHint>);

impl Frame {
    pub(crate) fn set_partial_hint(
        &mut self,
        slot: SlotId,
        written: Arc<WrittenType>,
        construction: Option<SlotId>,
    ) -> Result<(), Diagnostic> {
        self.set_partial_hint_with(slot, written, construction.map(PartialArguments::Construction))
    }
    pub(crate) fn set_partial_hint_with(
        &mut self,
        slot: SlotId,
        written: Arc<WrittenType>,
        arguments: Option<PartialArguments>,
    ) -> Result<(), Diagnostic> {
        self.slots
            .get(slot.0 as usize)
            .ok_or_else(|| invalid("partial hint slot absent"))?;
        self.partial_hints
            .0
            .insert(slot, PartialHint { written, arguments });
        Ok(())
    }
    pub(crate) fn clear_partial_hint(&mut self, slot: SlotId) {
        self.partial_hints.0.remove(&slot);
    }
    /// The type arguments a partial hint's construction has fixed so far.
    fn partial_arguments(
        &self,
        hint: &PartialHint,
        store: &GuestStore,
    ) -> Result<Vec<Option<TypeId>>, Diagnostic> {
        let guard = match &hint.arguments {
            None => return Ok(Vec::new()),
            Some(PartialArguments::Fixed(fixed)) => return Ok(fixed.to_vec()),
            Some(PartialArguments::Construction(guard)) => *guard,
        };
        let Some(active) = self
            .slots
            .get(guard.0 as usize)
            .and_then(|slot| slot.construction.as_ref())
        else {
            return Ok(Vec::new());
        };
        store.pending_construction_args(active.guard)
    }
    /// The exact descriptor of `written` under the fixed arguments, when no
    /// part of it is open.
    fn partial_exact(
        written: &WrittenType,
        args: &[Option<TypeId>],
        store: &mut GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        if !written.coercion_free() {
            return Ok(None);
        }
        written.materialize_with(store, &|index| args.get(index as usize).copied().flatten())
    }
    /// The type arguments a generic nominal's partial result context fixes
    /// (`Result<_, u32>` for `Ok(3)` at a `Result<T, u32>` parameter): its
    /// exactly known written arguments, the rest open. `None` without a
    /// partial context naming the same declaration.
    pub(crate) fn partial_nominal_args(
        &self,
        slot: SlotId,
        bound: &crate::frontend::types::BoundNominalVariant,
        store: &mut GuestStore,
    ) -> Result<Option<Vec<Option<TypeId>>>, Diagnostic> {
        let Some(hint) = self.partial_hints.0.get(&slot).cloned() else {
            return Ok(None);
        };
        let WrittenType::Generic { identity, args, .. } = hint.written.as_ref() else {
            return Ok(None);
        };
        if *identity != bound.identity() || args.len() != bound.type_parameters() as usize {
            return Ok(None);
        }
        let fixed = self.partial_arguments(&hint, store)?;
        let mut seeded = Vec::new();
        seeded
            .try_reserve_exact(args.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for arg in args {
            seeded.push(Self::partial_exact(arg, &fixed, store)?);
        }
        Ok(Some(seeded))
    }
    /// A borrow's operand under its result's partial reference hint
    /// (`&[u8]`, `&(_, u8)`): the pointee's exact Type as the operand hint,
    /// else the pointee as the operand's own partial hint.
    pub(crate) fn borrow_partial_hint(
        &mut self,
        reference: SlotId,
        operand: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let Some(hint) = self.partial_hints.0.get(&reference).cloned() else {
            return Ok(());
        };
        let WrittenType::Ref { target, .. } = hint.written.as_ref() else {
            return Ok(());
        };
        let args = self.partial_arguments(&hint, store)?;
        if let Some(ty) = Self::partial_exact(target, &args, store)? {
            return self.declared_hint(operand, Some(ty));
        }
        if !matches!(
            target.as_ref(),
            WrittenType::Infer | WrittenType::Parameter(_)
        ) {
            self.set_partial_hint_with(
                operand,
                Arc::new(target.as_ref().clone()),
                hint.arguments.clone(),
            )?;
        }
        Ok(())
    }
    /// An observed tuple/array constructor under its result's partial hint:
    /// members with an exactly known Type are hinted, nested partial members
    /// carry their own partial hint, and open members stay observed.
    pub(crate) fn prepare_partially_hinted_constructor(
        &mut self,
        plan: &crate::syntax::constructors::ConstructorPlan,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::syntax::constructors::ConstructorKind as C;
        let guard = self.admit_constructor_plan(plan)?;
        let result = self
            .body
            .constructor_node(guard)?
            .value
            .ok_or_else(|| invalid("observed constructor result absent"))?;
        let Some(hint) = self.partial_hints.0.remove(&result) else {
            return self.prepare_observed_structural_constructor(plan);
        };
        let members: Vec<(SlotId, &WrittenType)> = match (plan.kind(), hint.written.as_ref()) {
            (C::Tuple(fields), WrittenType::Tuple(written)) if fields.len() == written.len() => {
                fields
                    .iter()
                    .zip(written)
                    .map(|(field, member)| (field.value, member))
                    .collect()
            }
            (C::Array(fields), WrittenType::Array(element, len))
                if u32::try_from(fields.len()).ok() == Some(*len) =>
            {
                fields
                    .iter()
                    .map(|field| (field.value, element.as_ref()))
                    .collect()
            }
            // An array reached through unsizing to a written slice.
            (C::Array(fields), WrittenType::Slice(element)) => fields
                .iter()
                .map(|field| (field.value, element.as_ref()))
                .collect(),
            _ => Vec::new(),
        };
        let args = self.partial_arguments(&hint, store)?;
        let mut hints = Vec::new();
        let mut nested = Vec::new();
        for (slot, member) in members {
            if let Some(ty) = Self::partial_exact(member, &args, store)? {
                hints.push((slot, ty));
            } else if !matches!(member, WrittenType::Infer | WrittenType::Parameter(_)) {
                nested.push((slot, Arc::new(member.clone())));
            }
        }
        self.prepare_observed_structural_constructor_with(plan, hints)?;
        for (slot, written) in nested {
            self.set_partial_hint_with(slot, written, hint.arguments.clone())?;
        }
        Ok(())
    }
}

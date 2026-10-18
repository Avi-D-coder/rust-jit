//! Readonly runtime evidence for retained ordinary source type equalities.
use super::*;
use crate::ir::op::{NumericTypeProjection as P, RelatedNumericPlan};
use crate::runtime::shape::TypeKey;

impl Frame {
    pub(crate) fn related_numeric_hint(
        &self,
        plan: &RelatedNumericPlan,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let mut answer = None;
        for source in plan.sources.iter() {
            if source.node.0 == self.pc.node {
                continue;
            }
            let Some(value) = self
                .body
                .nodes()
                .get(source.node.0 as usize)
                .and_then(|node| node.value)
            else {
                continue;
            };
            let slot = self.slot(value)?;
            if !self.active_scopes.contains(&slot.scope) {
                continue;
            }
            let Some(cell) = slot.cell else {
                continue;
            };
            let CellState::Live(value) = store.state(cell)? else {
                continue;
            };
            let Some(mut ty) = store.effective_type(value)? else {
                continue;
            };
            let mut projected = true;
            for step in &source.projections {
                if *step == P::AutoDeref {
                    loop {
                        match store.type_descriptor(ty)?.key.as_ref() {
                            TypeKey::Ref { target, .. } => ty = *target,
                            _ => match store.box_contents_type(ty)? {
                                Some(contents) => ty = contents,
                                None => {
                                    // A nominal may take a source-defined
                                    // Deref step. Its own type arguments are
                                    // not evidence for that target's arguments.
                                    if matches!(
                                        store.type_descriptor(ty)?.key.as_ref(),
                                        TypeKey::Nominal { .. }
                                    ) {
                                        projected = false;
                                    }
                                    break;
                                }
                            },
                        }
                    }
                    if !projected {
                        break;
                    }
                    continue;
                }
                let next = match (step, store.type_descriptor(ty)?.key.as_ref()) {
                    (P::Deref, TypeKey::Ref { target, .. }) => Some(*target),
                    (P::Deref, _) => store.box_contents_type(ty)?,
                    (P::TupleField(field), TypeKey::Tuple(fields)) => {
                        fields.get(*field as usize).copied()
                    }
                    (P::Element, TypeKey::Array { element, .. } | TypeKey::Slice { element }) => {
                        Some(*element)
                    }
                    (P::TypeArgument(position), TypeKey::Nominal { args, .. }) => {
                        args.get(*position as usize).copied()
                    }
                    _ => None,
                };
                match next {
                    Some(next) => ty = next,
                    None => {
                        projected = false;
                        break;
                    }
                }
            }
            if !projected {
                continue;
            }
            // Builtin scalar operators accept a shared-reference operand of
            // the same primitive, as the common source relation states.
            if let TypeKey::Ref { target, .. } = store.type_descriptor(ty)?.key.as_ref() {
                ty = *target;
            }
            let Some(crate::traits::term::TyTerm::Primitive(primitive)) =
                store.descriptor_source_term(ty)
            else {
                continue;
            };
            if Self::primitive_numeric_family(primitive) != Some(plan.floating) {
                continue;
            }
            if answer.is_some_and(|prior| prior != ty) {
                return Ok(None);
            }
            answer = Some(ty);
        }
        Ok(answer)
    }
}

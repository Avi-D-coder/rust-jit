//! Membership checks for already admitted structural source types. A runtime
//! descriptor never constructs a source type or resolves a written Type path.
use super::*;
use crate::{
    runtime::shape::TypeKey,
    traits::term::{ConstTerm, TyTerm},
};

impl Frame {
    pub(super) fn supported_structural_source(source: &TyTerm) -> bool {
        Self::structural_source_shape(source)
    }
    fn structural_source_shape(source: &TyTerm) -> bool {
        match source {
            TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Dyn(_) | TyTerm::Param(_) => true,
            TyTerm::Tuple(members) => members.iter().all(Self::structural_source_shape),
            // A reference member (`(&K, &V)`): regions are not data.
            TyTerm::Ref(_, _, inner) => Self::structural_source_shape(inner),
            TyTerm::Slice(element) => Self::structural_source_shape(element),
            TyTerm::Array(element, ConstTerm::Value(length)) => {
                u32::try_from(*length).is_ok() && Self::structural_source_shape(element)
            }
            // A selected nominal member (`(i32, P)`, `(u8, W<i32>)`): its
            // declaration identity and closed type arguments.
            TyTerm::Nominal(_, args) => args.iter().all(|arg| {
                matches!(arg, crate::traits::term::GenericArg::Type(ty)
                    if Self::structural_source_shape(ty))
            }),
            // A closed fn-pointer member (E-R5-008).
            TyTerm::Function(_) => crate::frontend::call_contracts::closed_term(source),
            _ => false,
        }
    }
    pub(super) fn require_structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        Self::structural_source_target(source, target, store, None)
    }
    pub(super) fn require_bound_structural_source_target(
        &self,
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        Self::structural_source_target(source, target, store, self.context.as_ref())
    }
    fn structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
        context: Option<&crate::vm::context::GenericEnv>,
    ) -> Result<(), Diagnostic> {
        let actual = store.type_descriptor(target)?;
        let mismatch = || {
            Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "structural source differs from its target descriptor".into(),
            )
        };
        // A declared `dyn Trait` target position admits any source view.
        if matches!(actual.key.as_ref(), TypeKey::Dyn { .. }) {
            return Ok(());
        }
        // A raw pointer or fn pointer view matches any descriptor of its
        // family; the pointee may differ after a pointer cast (E-R5-008).
        if crate::traits::pointer_terms::as_raw_pointer(source).is_some() {
            return match actual.key.as_ref() {
                TypeKey::RawPtr { .. } => Ok(()),
                _ => Err(mismatch()),
            };
        }
        if matches!(source, TyTerm::Function(_)) {
            return match actual.key.as_ref() {
                TypeKey::Function { .. } => Ok(()),
                _ => Err(mismatch()),
            };
        }
        match source {
            // A declared `dyn Trait` position admits the value's concrete Type.
            TyTerm::Dyn(_) => {}
            TyTerm::Param(key) => {
                let bound = context
                    .and_then(|context| context.runtime_binding(*key))
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "source parameter has no accepted runtime binding".into(),
                        )
                    })?;
                if bound != target {
                    return Err(mismatch());
                }
            }
            TyTerm::Primitive(primitive) => {
                if crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id() != target
                {
                    return Err(mismatch());
                }
            }
            TyTerm::Str => {
                if target != TypeId(16) {
                    return Err(mismatch());
                }
            }
            TyTerm::Slice(element) => {
                let TypeKey::Slice {
                    element: actual_element,
                } = actual.key.as_ref()
                else {
                    return Err(mismatch());
                };
                Self::structural_source_target(element, *actual_element, store, context)?;
            }
            TyTerm::Ref(_, mutable, inner) => {
                let TypeKey::Ref {
                    mutable: actual_mutable,
                    target: actual_target,
                } = actual.key.as_ref()
                else {
                    return Err(mismatch());
                };
                if actual_mutable != mutable {
                    return Err(mismatch());
                }
                Self::structural_source_target(inner, *actual_target, store, context)?;
            }
            TyTerm::Tuple(members) if members.is_empty() => {
                if target != TypeId(1) {
                    return Err(mismatch());
                }
            }
            TyTerm::Tuple(members) => {
                let TypeKey::Tuple(fields) = actual.key.as_ref() else {
                    return Err(mismatch());
                };
                if members.len() != fields.len() {
                    return Err(mismatch());
                }
                for (member, field) in members.iter().zip(fields.iter()) {
                    Self::structural_source_target(member, *field, store, context)?;
                }
            }
            TyTerm::Array(element, ConstTerm::Value(length)) => {
                let TypeKey::Array {
                    element: actual_element,
                    len,
                } = actual.key.as_ref()
                else {
                    return Err(mismatch());
                };
                if u64::from(*len) != *length {
                    return Err(mismatch());
                }
                Self::structural_source_target(element, *actual_element, store, context)?;
            }
            TyTerm::Nominal(identity, arguments) => {
                let TypeKey::Nominal {
                    identity: actual,
                    args,
                    const_args,
                    ..
                } = actual.key.as_ref()
                else {
                    return Err(mismatch());
                };
                if actual != identity || args.len() != arguments.len() || !const_args.is_empty() {
                    return Err(mismatch());
                }
                for (argument, arg) in arguments.iter().zip(args.iter()) {
                    let crate::traits::term::GenericArg::Type(argument) = argument else {
                        return Err(mismatch());
                    };
                    Self::structural_source_target(argument, *arg, store, context)?;
                }
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "structural member requires its admitted source type proof".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "frame_source_types_tests.rs"]
mod tests;

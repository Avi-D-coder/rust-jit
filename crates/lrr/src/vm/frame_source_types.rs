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
            TyTerm::Primitive(_) => true,
            TyTerm::Tuple(members) => members.iter().all(Self::structural_source_shape),
            TyTerm::Array(element, ConstTerm::Value(length)) => {
                u32::try_from(*length).is_ok() && Self::structural_source_shape(element)
            }
            // A selected nominal member (`(i32, P)`, `(u8, W<i32>)`): its
            // declaration identity and closed type arguments.
            TyTerm::Nominal(_, args) => args.iter().all(|arg| {
                matches!(arg, crate::traits::term::GenericArg::Type(ty)
                    if Self::structural_source_shape(ty))
            }),
            _ => false,
        }
    }
    pub(super) fn require_structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        Self::structural_source_target(source, target, store)
    }
    fn structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let actual = store.type_descriptor(target)?;
        let mismatch = || {
            Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "structural source differs from its target descriptor".into(),
            )
        };
        match source {
            TyTerm::Primitive(primitive) => {
                if crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id() != target
                {
                    return Err(mismatch());
                }
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
                    Self::structural_source_target(member, *field, store)?;
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
                Self::structural_source_target(element, *actual_element, store)?;
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
                    Self::structural_source_target(argument, *arg, store)?;
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

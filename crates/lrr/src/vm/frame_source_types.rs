//! Membership checks for already admitted structural source types. A runtime
//! descriptor never constructs a source type or resolves a written Type path.
use super::*;
use crate::{
    runtime::shape::TypeKey,
    traits::term::{ConstTerm, TyTerm},
};

impl Frame {
    pub(super) fn supported_structural_source(source: &TyTerm) -> bool {
        Self::structural_source_shape(source, 0, &mut 0)
    }
    fn structural_source_shape(source: &TyTerm, depth: usize, work: &mut usize) -> bool {
        let Some(next) = work.checked_add(1).filter(|count| *count <= 100_000) else {
            return false;
        };
        *work = next;
        if depth >= 256 {
            return false;
        }
        match source {
            TyTerm::Primitive(_) => true,
            TyTerm::Tuple(members) => members
                .iter()
                .all(|member| Self::structural_source_shape(member, depth + 1, work)),
            TyTerm::Array(element, ConstTerm::Value(length)) => {
                u32::try_from(*length).is_ok()
                    && Self::structural_source_shape(element, depth + 1, work)
            }
            _ => false,
        }
    }
    pub(super) fn require_structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        Self::structural_source_target(source, target, store, 0, &mut 0)
    }
    fn structural_source_target(
        source: &TyTerm,
        target: TypeId,
        store: &GuestStore,
        depth: usize,
        work: &mut usize,
    ) -> Result<(), Diagnostic> {
        *work = work
            .checked_add(1)
            .filter(|count| *count <= 100_000)
            .ok_or_else(crate::runtime::arena::resource)?;
        if depth >= 256 {
            return Err(crate::runtime::arena::resource());
        }
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
                    Self::structural_source_target(member, *field, store, depth + 1, work)?;
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
                Self::structural_source_target(element, *actual_element, store, depth + 1, work)?;
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
#[path = "frame_pattern_decision_tests.rs"]
mod pattern_decision_tests;
#[cfg(test)]
#[path = "frame_source_types_tests.rs"]
mod tests;

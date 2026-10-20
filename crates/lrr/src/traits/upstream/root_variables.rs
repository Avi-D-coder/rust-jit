//! Owned source inference keys joined to one actual pinned inference owner.
//! No solver, canonicalization, source query or runtime mutation occurs here.
use super::{
    infer::delegate::SolverContext,
    model::{Span, Ty},
    prepared::{Cx, PreparedProvider},
    source_terms::SourceTypePending,
};
use crate::{core::diagnostic::Diagnostic, traits::term::InferVarKey};
use std::collections::BTreeMap;

pub(super) use super::root_input::SourceVariableFamily as RootVariableKind;

/// This map cannot outlive its inference owner. A source key denotes the same
/// variable at every occurrence, and its numeric family survives encoding.
pub(super) struct RootVariables<'owner, 'cx> {
    owner: &'owner SolverContext<'cx>,
    variables: BTreeMap<InferVarKey, Ty<'cx, PreparedProvider<'cx>>>,
    specifications: BTreeMap<InferVarKey, RootVariableKind>,
}
impl<'owner, 'cx> RootVariables<'owner, 'cx> {
    pub(super) fn new(
        owner: &'owner SolverContext<'cx>,
        specifications: impl IntoIterator<Item = (InferVarKey, RootVariableKind)>,
        span: Span,
    ) -> Result<Self, Diagnostic> {
        // Validate the owned input before allocating any inference state.
        let mut kinds = BTreeMap::new();
        for (key, kind) in specifications {
            if kinds.insert(key, kind).is_some() {
                return Err(Diagnostic::new(
                    "LRR-E-UPSTREAM-SOLVER-INPUT",
                    "duplicate source inference key in root input".into(),
                ));
            }
        }
        let variables = kinds
            .iter()
            .map(|(&key, &kind)| {
                let value = match kind {
                    RootVariableKind::Type => owner.next_ty_var(span),
                    RootVariableKind::Integer => owner.next_int_var(),
                    RootVariableKind::Float => owner.next_float_var(),
                };
                (key, value)
            })
            .collect();
        Ok(Self {
            owner,
            variables,
            specifications: kinds,
        })
    }

    pub(super) fn specifications(&self) -> &BTreeMap<InferVarKey, RootVariableKind> {
        &self.specifications
    }

    pub(super) fn require_owner(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<(), SourceTypePending> {
        if std::ptr::eq(self.owner, owner) {
            Ok(())
        } else {
            Err(SourceTypePending::InferenceOwner)
        }
    }

    pub(super) fn context(&self) -> Cx<'cx> {
        self.owner.interner
    }

    pub(super) fn variable(
        &self,
        key: InferVarKey,
    ) -> Result<Ty<'cx, PreparedProvider<'cx>>, SourceTypePending> {
        self.variables
            .get(&key)
            .copied()
            .ok_or(SourceTypePending::Inference)
    }

    /// Root evaluation applies its canonical response to this owner. Read
    /// these original variables afterward; GoalEvaluation.goal is not that
    /// response. Returned IR stays scoped, pending an owned response export.
    pub(super) fn resolved(
        &self,
        key: InferVarKey,
    ) -> Result<Ty<'cx, PreparedProvider<'cx>>, SourceTypePending> {
        Ok(self
            .owner
            .deeply_resolve_ignoring_regions(self.variable(key)?))
    }
}

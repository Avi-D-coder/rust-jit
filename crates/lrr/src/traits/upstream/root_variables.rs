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
type RootConst<'cx> = rustc_type_ir::Const<Cx<'cx>>;

/// This map cannot outlive its inference owner. A source key denotes the same
/// variable at every occurrence, and its numeric family survives encoding.
pub(super) struct RootVariables<'owner, 'cx> {
    owner: &'owner SolverContext<'cx>,
    variables: BTreeMap<InferVarKey, Ty<'cx, PreparedProvider<'cx>>>,
    specifications: BTreeMap<InferVarKey, RootVariableKind>,
    /// Const inference variables (array lengths a callee's const parameter
    /// states): they have no Type family, so they are not specifications.
    consts: BTreeMap<InferVarKey, RootConst<'cx>>,
}
impl<'owner, 'cx> RootVariables<'owner, 'cx> {
    pub(super) fn new(
        owner: &'owner SolverContext<'cx>,
        specifications: impl IntoIterator<Item = (InferVarKey, RootVariableKind)>,
        span: Span,
    ) -> Result<Self, Diagnostic> {
        Self::with_consts(owner, specifications, std::iter::empty(), span)
    }

    pub(super) fn with_consts(
        owner: &'owner SolverContext<'cx>,
        specifications: impl IntoIterator<Item = (InferVarKey, RootVariableKind)>,
        consts: impl IntoIterator<Item = InferVarKey>,
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
        let mut const_variables = BTreeMap::new();
        for key in consts {
            if kinds.contains_key(&key) || const_variables.insert(key, owner.next_const_var(span)).is_some() {
                return Err(Diagnostic::new(
                    "LRR-E-UPSTREAM-SOLVER-INPUT",
                    "duplicate source inference key in root input".into(),
                ));
            }
        }
        Ok(Self {
            owner,
            variables,
            specifications: kinds,
            consts: const_variables,
        })
    }

    pub(super) fn const_variable(
        &self,
        key: InferVarKey,
    ) -> Result<RootConst<'cx>, SourceTypePending> {
        self.consts
            .get(&key)
            .copied()
            .ok_or(SourceTypePending::Inference)
    }

    pub(super) fn const_keys(&self) -> impl Iterator<Item = InferVarKey> + '_ {
        self.consts.keys().copied()
    }

    pub(super) fn resolved_const(
        &self,
        key: InferVarKey,
    ) -> Result<RootConst<'cx>, SourceTypePending> {
        Ok(self
            .owner
            .deeply_resolve_ignoring_regions(self.const_variable(key)?))
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

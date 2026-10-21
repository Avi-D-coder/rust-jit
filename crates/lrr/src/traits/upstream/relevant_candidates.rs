//! Candidate-dependency closure restricted to the impls pinned selection can
//! actually visit. Metadata only: no candidate is rejected, no answer is
//! produced and no builtin callback is added.
//!
//! Pinned `assemble_candidates_after_normalizing_self_ty` normalizes an alias
//! `Self` type with the solver's own `NormalizationFolder` before it calls
//! `for_each_relevant_impl`, which visits one simplified-type group plus the
//! blanket impls. The source coordinator must prepare callback dependencies for
//! exactly that traversal, not for every member of a complete trait bucket:
//! a foreign bucket member's child bounds (for example `Copy` on a container
//! impl) are never read for a scalar goal.
use super::{
    identity::DefinitionId,
    infer::{InferenceContext, delegate::SolverContext},
    model::{self, Span},
    prepared::{Cx, PreparedProvider},
    reached_call::SourceRootAdmission,
    source_definition_headers::DefinitionHeaderRole,
    source_fact_bundle::{SourceFactBundle, SourceImplFacts},
    source_runtime_demands::SourceRuntimeDemands,
};
use crate::core::digest::Digest;
use rustc_next_trait_solver::{
    normalize::{NormalizationFolder, NormalizationWasAmbiguous},
    solve::SolverDelegateEvalExt,
};
use rustc_type_ir::{
    self as ty, Interner, TypeFoldable, TypeVisitableExt, Upcast,
    inherent::*,
    solve::{Certainty, Goal},
};
use std::collections::{BTreeMap, BTreeSet};

type RootTy<'cx> = model::Ty<'cx, PreparedProvider<'cx>>;

/// Trait definitions whose callback dependencies the actual selected numeric
/// clauses may read through relevant impls. `Self` types containing aliases are
/// normalized first, exactly as the pinned solver does. A `Self` the adapter
/// cannot normalize outside the owner (it mentions inference variables) keeps
/// the complete bucket, never an empty answer.
pub(super) fn relevant_candidate_traits<'cx>(
    cx: Cx<'cx>,
    admission: &SourceRootAdmission,
    owner: &SolverContext<'cx>,
    demands: &SourceRuntimeDemands<'_, '_, 'cx>,
    bundle: &SourceFactBundle,
    span: Span,
) -> Result<BTreeSet<Digest>, String> {
    let provider: PreparedProvider<'cx> = cx.services();
    let by_canonical: BTreeMap<Digest, &SourceImplFacts> = bundle
        .implementations()
        .values()
        .map(|facts| (facts.canonical(), facts))
        .collect();
    let mut traits = BTreeSet::new();
    for numeric in demands.clauses() {
        let ty::ClauseKind::Trait(bound) = numeric.clause().kind().skip_binder() else {
            continue;
        };
        let trait_id = bound.trait_ref.def_id;
        let self_ty = owner.deeply_resolve_ignoring_regions(bound.trait_ref.self_ty());
        let mut visited: Vec<&SourceImplFacts> = Vec::new();
        match normalize_for_selection(cx, admission, demands.param_env(), self_ty, span)? {
            Some(self_ty) => {
                let mut missing = None;
                provider.for_each_relevant_source_impl(cx, trait_id, self_ty, |id: DefinitionId| {
                    match by_canonical.get(&id.source()) {
                        Some(facts) => visited.push(facts),
                        None => missing = Some(id),
                    }
                });
                if let Some(id) = missing {
                    return Err(format!(
                        "relevant impl {id:?} is not a retained candidate of the demanded bucket"
                    ));
                }
            }
            None => {
                for (raw, _) in bundle.candidates(trait_id.source()).unwrap_or_default() {
                    visited.push(
                        bundle
                            .implementations()
                            .get(raw)
                            .ok_or_else(|| "candidate impl facts were not retained".to_string())?,
                    );
                }
            }
        }
        for facts in visited {
            traits.extend(impl_clause_traits(bundle, facts));
        }
    }
    Ok(traits)
}

/// Trait definitions named by one impl's complete clause bindings. Identities
/// only: the clauses are neither evaluated nor filtered.
pub(super) fn impl_clause_traits<'a>(
    bundle: &'a SourceFactBundle,
    facts: &'a SourceImplFacts,
) -> impl Iterator<Item = Digest> + 'a {
    facts
        .semantics()
        .full_clause_bindings()
        .into_iter()
        .flat_map(|bindings| bindings.definitions.keys().copied())
        .filter(|source| {
            bundle
                .definitions()
                .get(source)
                .is_some_and(|facts| facts.header().role() == DefinitionHeaderRole::Trait)
        })
}

/// The conservative form for a trait whose goals are not yet known: every
/// member of its complete bucket. Used only for seeds that no selected `Self`
/// type could be normalized for.
pub(super) fn bucket_clause_traits(bundle: &SourceFactBundle, trait_definition: Digest) -> BTreeSet<Digest> {
    let mut traits = BTreeSet::new();
    for (raw, _) in bundle.candidates(trait_definition).unwrap_or_default() {
        if let Some(facts) = bundle.implementations().get(raw) {
            traits.extend(impl_clause_traits(bundle, facts));
        }
    }
    traits
}

/// The solver's own normalization of an alias-containing `Self`, in a throwaway
/// inference owner admitted under the same private source root. `None` means
/// the type cannot be normalized away from the root owner's variables.
fn normalize_for_selection<'cx>(
    cx: Cx<'cx>,
    admission: &SourceRootAdmission,
    param_env: model::ParamEnv<'cx, PreparedProvider<'cx>>,
    self_ty: RootTy<'cx>,
    span: Span,
) -> Result<Option<RootTy<'cx>>, String> {
    if !cx.renormalize_rigid_aliases() && !self_ty.has_non_rigid_aliases() {
        return Ok(Some(self_ty));
    }
    if !self_ty.has_aliases() {
        return Ok(Some(self_ty));
    }
    if self_ty.has_infer() {
        return Ok(None);
    }
    let probe = SolverContext::from_inference_context(InferenceContext::new(
        cx,
        ty::TypingMode::PostAnalysis,
    ));
    probe.admit_source_root(admission);
    let infcx: &InferenceContext<'cx> = &probe;
    let mut folder = NormalizationFolder::new(infcx, vec![], |alias_term: ty::AliasTerm<Cx<'cx>>| {
        let infer: model::Term<'cx, PreparedProvider<'cx>> = match alias_term.kind {
            ty::AliasTermKind::ProjectionTy { .. }
            | ty::AliasTermKind::InherentTy { .. }
            | ty::AliasTermKind::OpaqueTy { .. }
            | ty::AliasTermKind::FreeTy { .. } => probe.next_ty_var(span).into(),
            _ => probe.next_const_var(span).into(),
        };
        let predicate: model::Predicate<'cx, PreparedProvider<'cx>> = ty::ProjectionClause {
            projection_term: alias_term,
            term: infer,
        }
        .upcast(cx);
        let goal = Goal::new(cx, param_env, predicate);
        let evaluation = probe
            .evaluate_root_goal(goal, span, None)
            .map_err(|_| "alias normalization goal has no solution".to_string())?;
        let ambiguous = match evaluation.certainty {
            Certainty::Yes => NormalizationWasAmbiguous::No,
            Certainty::Maybe(_) => NormalizationWasAmbiguous::Yes,
        };
        Ok((probe.deeply_resolve_ignoring_regions(infer), ambiguous))
    });
    self_ty.try_fold_with(&mut folder).map(Some)
}

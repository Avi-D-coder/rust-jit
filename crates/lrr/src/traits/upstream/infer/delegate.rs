//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::canonical::CanonicalExt;
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Span, Ty};
use crate::traits::upstream::{identity::DefinitionId, model};
use rustc_next_trait_solver::delegate::SolverDelegate;
use rustc_type_ir::solve::{
    Certainty, ComputeGoalFastPathOutcome, FetchEligibleAssocItemResponse, Goal, NoSolution,
    VisibleForLeakCheck,
};
use rustc_type_ir::{self as ty, TypeFoldable, inherent::*};
use rustc_type_ir::{MayBeErased, TypeVisitableExt, TypingMode};
use std::{fmt::Debug, ops::Deref};
type GenericArg<'db> = model::Arg<'db, PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db, PreparedProvider<'db>>;
type Term<'db> = model::Term<'db, PreparedProvider<'db>>;
type Args<'db> = model::Args<'db, PreparedProvider<'db>>;
type Canonical<'db, V> = ty::Canonical<DbInterner<'db>, V>;
type CanonicalVarValues<'db> = ty::CanonicalVarValues<DbInterner<'db>>;
type CanonicalVarKind<'db> = ty::CanonicalVarKind<DbInterner<'db>>;

/// Required real compiler-client functions over admitted immutable source facts.
/// No Default, test stub, guest-loaded function or implicit incomplete answer.
/// Implementing/installing these clients remains a separate root-entry gate.
pub(crate) struct SolverClients<'db> {
    pub(crate) transmutable:
        fn(&InferenceContext<'db>, Ty<'db>, Ty<'db>, Const<'db>) -> Result<Certainty, NoSolution>,
}

/// Own exactly one inference context. Safe ownership replaces the upstream
/// transparent borrowed-reference transmute; there is no second mutable store.
pub(crate) struct SolverContext<'db>(InferenceContext<'db>);
impl<'db> SolverContext<'db> {
    pub(crate) fn from_inference_context(context: InferenceContext<'db>) -> Self {
        Self(context)
    }
}
impl<'db> Deref for SolverContext<'db> {
    type Target = InferenceContext<'db>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'db> SolverDelegate for SolverContext<'db> {
    type Infcx = InferenceContext<'db>;
    type Interner = DbInterner<'db>;
    fn build_with_canonical<V>(
        cx: Self::Interner,
        input: &ty::CanonicalQueryInput<Self::Interner, V>,
    ) -> (Self, V, CanonicalVarValues<'db>)
    where
        V: TypeFoldable<Self::Interner>,
    {
        let span = <Span as ty::inherent::Span<Self::Interner>>::dummy();
        let (context, value, variables) = InferenceContext::build_with_canonical(cx, span, input);
        (Self(context), value, variables)
    }
    fn compute_goal_fast_path(
        &self,
        goal: Goal<Self::Interner, Predicate<'db>>,
        span: Span,
    ) -> ComputeGoalFastPathOutcome<Self::Interner> {
        self.0.compute_goal_fast_path(goal, span)
    }
    fn fresh_var_for_kind(
        &self,
        arg: GenericArg<'db>,
        span: Span,
        universe: ty::UniverseIndex,
    ) -> GenericArg<'db> {
        match arg.kind() {
            ty::GenericArgKind::Lifetime(_) => {
                self.next_region_var_in_universe(universe, span).into()
            }
            ty::GenericArgKind::Type(_) => self.next_ty_var_in_universe(universe, span).into(),
            ty::GenericArgKind::Const(_) => self.next_const_var_in_universe(universe, span).into(),
        }
    }
    fn leak_check(&self, max_input_universe: ty::UniverseIndex) -> Result<(), NoSolution> {
        self.0
            .inner
            .borrow_mut()
            .unwrap_region_constraints()
            .leak_check(self.interner, max_input_universe, self.universe(), None)
            .map_err(|_| NoSolution)
    }
    fn instantiate_canonical<V>(
        &self,
        canonical: Canonical<'db, V>,
        values: CanonicalVarValues<'db>,
    ) -> V
    where
        V: TypeFoldable<DbInterner<'db>>,
    {
        canonical.instantiate(self.interner, &values)
    }
    fn instantiate_canonical_var(
        &self,
        kind: CanonicalVarKind<'db>,
        span: Span,
        var_values: &[GenericArg<'db>],
        universe_map: impl Fn(ty::UniverseIndex) -> ty::UniverseIndex,
    ) -> GenericArg<'db> {
        self.0
            .instantiate_canonical_var(span, kind, var_values, universe_map)
    }
    fn fetch_eligible_assoc_item(
        &self,
        goal_trait_ref: ty::TraitRef<Self::Interner>,
        trait_assoc_def_id: DefinitionId,
        impl_def_id: DefinitionId,
    ) -> FetchEligibleAssocItemResponse<Self::Interner> {
        let node_item = match self
            .interner
            .services()
            .associated_leaf(impl_def_id, trait_assoc_def_id)
        {
            Ok(i) => i,
            Err(guar) => return FetchEligibleAssocItemResponse::Err(guar),
        };

        let typing_mode = self.typing_mode_raw();

        let eligible = if node_item.is_final() {
            // Non-specializable items are always projectable.
            true
        } else {
            // Only reveal a specializable default if we're past type-checking
            // and the obligation is monomorphic, otherwise passes such as
            // transmute checking and polymorphic MIR optimizations could
            // get a result which isn't correct for all monomorphizations.
            match typing_mode {
                TypingMode::Coherence
                | TypingMode::Typeck { .. }
                | TypingMode::PostTypeckUntilBorrowck { .. }
                | TypingMode::Reflection
                | TypingMode::PostBorrowck { .. } => false,
                TypingMode::PostAnalysis | TypingMode::Codegen => {
                    let poly_trait_ref = self.deeply_resolve_ignoring_regions(goal_trait_ref);
                    !poly_trait_ref.still_further_specializable()
                }
                TypingMode::ErasedNotCoherence(MayBeErased) => {
                    return FetchEligibleAssocItemResponse::NotFoundBecauseErased;
                }
            }
        };

        // FIXME: Check for defaultness here may cause diagnostics problems.
        if eligible {
            FetchEligibleAssocItemResponse::Found(node_item.item)
        } else {
            // We know it's not erased since then we'd have returned in the match above,
            // or node_item.final() was true and eligible is always true.
            FetchEligibleAssocItemResponse::NotFound(typing_mode.assert_not_erased())
        }
    }

    fn evaluate_const<E: Debug>(
        &self,
        env: ParamEnv<'db>,
        alias: ty::AliasConst<Self::Interner>,
        normalize: impl FnOnce(ty::Unnormalized<Self::Interner, Ty<'db>>) -> Result<Ty<'db>, E>,
    ) -> Result<Option<Const<'db>>, E> {
        super::const_eval::evaluate_const(&self.0, env, alias, normalize).unwrap_or_else(
            |dependency| {
                panic!(
                    "const client entered before required immutable input admission: {dependency:?}"
                )
            },
        )
    }

    fn well_formed_goals(
        &self,
        env: ParamEnv<'db>,
        term: Term<'db>,
    ) -> Option<Vec<Goal<Self::Interner, Predicate<'db>>>> {
        super::well_formed::unnormalized_obligations(
            &self.0,
            env,
            term,
            <Span as ty::inherent::Span<Self::Interner>>::dummy(),
        )
        .map(|obligations| {
            obligations
                .into_iter()
                .map(|obligation| obligation.as_goal())
                .collect()
        })
    }
    fn make_deduplicated_region_constraints(
        &self,
    ) -> Vec<(ty::RegionConstraint<Self::Interner>, VisibleForLeakCheck)> {
        self.0.make_deduplicated_region_constraints()
    }
    fn add_item_bounds_for_hidden_type(
        &self,
        id: DefinitionId,
        args: Args<'db>,
        env: ParamEnv<'db>,
        hidden: Ty<'db>,
        goals: &mut Vec<Goal<Self::Interner, Predicate<'db>>>,
    ) {
        self.0
            .add_item_bounds_for_hidden_type(id, args, env, hidden, goals)
    }
    fn is_transmutable(
        &self,
        src: Ty<'db>,
        dst: Ty<'db>,
        assume: Const<'db>,
    ) -> Result<Certainty, NoSolution> {
        (self.interner.services().solver_clients().transmutable)(&self.0, src, dst, assume)
    }
    fn emit_next_solver_overflow_fcw(
        &self,
        goal: Goal<Self::Interner, Predicate<'db>>,
        span: Span,
    ) {
        // E006: no static lint enforcement. Retain non-enforcing diagnostic
        // evidence; exact rustc lint/proof-tree formatting is a separate gate.
        self.interner
            .services()
            .record_solver_overflow(span, format!("next solver overflow warning: {goal:?}"));
    }
}

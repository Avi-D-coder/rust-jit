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
    /// Err(()) means only the supplied normalization callback failed. Other
    /// evaluation errors are represented as the actual error Const, as upstream.
    pub(crate) evaluate_const: fn(
        &InferenceContext<'db>,
        ParamEnv<'db>,
        ty::AliasConst<DbInterner<'db>>,
        &mut dyn FnMut(ty::Unnormalized<DbInterner<'db>, Ty<'db>>) -> Option<Ty<'db>>,
    ) -> Result<Option<Const<'db>>, ()>,
    pub(crate) well_formed_goals: fn(
        &InferenceContext<'db>,
        ParamEnv<'db>,
        Term<'db>,
    ) -> Option<Vec<Goal<DbInterner<'db>, Predicate<'db>>>>,
    pub(crate) hidden_type_bounds: fn(
        &InferenceContext<'db>,
        DefinitionId,
        Args<'db>,
        ParamEnv<'db>,
        Ty<'db>,
        &mut Vec<Goal<DbInterner<'db>, Predicate<'db>>>,
    ),
    pub(crate) eligible_assoc_item: fn(
        &InferenceContext<'db>,
        ty::TraitRef<DbInterner<'db>>,
        DefinitionId,
        DefinitionId,
    ) -> FetchEligibleAssocItemResponse<DbInterner<'db>>,
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

    fn evaluate_const<E: Debug>(
        &self,
        env: ParamEnv<'db>,
        alias: ty::AliasConst<Self::Interner>,
        normalize: impl FnOnce(ty::Unnormalized<Self::Interner, Ty<'db>>) -> Result<Ty<'db>, E>,
    ) -> Result<Option<Const<'db>>, E> {
        let mut normalize = Some(normalize);
        let mut failure = None;
        let mut callback = |value| match normalize
            .take()
            .expect("upstream const normalization is FnOnce")(
            value
        ) {
            Ok(value) => Some(value),
            Err(error) => {
                failure = Some(error);
                None
            }
        };
        let result = (self.interner.services().solver_clients().evaluate_const)(
            &self.0,
            env,
            alias,
            &mut callback,
        );
        match result {
            Ok(value) => {
                assert!(
                    failure.is_none(),
                    "const client discarded normalization failure"
                );
                Ok(value)
            }
            Err(()) => Err(failure.expect("const client failed without normalization error")),
        }
    }
    fn well_formed_goals(
        &self,
        env: ParamEnv<'db>,
        term: Term<'db>,
    ) -> Option<Vec<Goal<Self::Interner, Predicate<'db>>>> {
        (self.interner.services().solver_clients().well_formed_goals)(&self.0, env, term)
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
        (self.interner.services().solver_clients().hidden_type_bounds)(
            &self.0, id, args, env, hidden, goals,
        )
    }
    fn fetch_eligible_assoc_item(
        &self,
        goal: ty::TraitRef<Self::Interner>,
        item: DefinitionId,
        implementation: DefinitionId,
    ) -> FetchEligibleAssocItemResponse<Self::Interner> {
        (self
            .interner
            .services()
            .solver_clients()
            .eligible_assoc_item)(&self.0, goal, item, implementation)
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

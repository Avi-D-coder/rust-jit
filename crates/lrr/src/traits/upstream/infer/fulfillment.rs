//! Pinned rustc next-solver multi-root fulfillment; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_fulfillment.py.
//! Exact originals/licenses: vendor/rustc-trait-solver. No compiler diagnostics.
use super::{DbInterner, PreparedProvider, Span, delegate::SolverContext};
use crate::traits::upstream::model;
use rustc_next_trait_solver::solve::fast_path::compute_goal_fast_path;
use rustc_next_trait_solver::solve::{GoalEvaluation, HasChanged, SolverDelegateEvalExt as _};
use rustc_type_ir::solve::{Certainty, Goal, GoalStalledOn, NoSolution};
use rustc_type_ir::{self as ty, InferCtxtLike, Interner, inherent::*};
use thin_vec::ThinVec;
type Predicate<'db> = model::Predicate<'db, PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type PredicateObligations<'db, C> = ThinVec<PredicateObligation<'db, C>>;
// Pinned ThinVec retains the compiler's hot retain_mut representation.
type PendingObligations<'db, C> = ThinVec<(
    PredicateObligation<'db, C>,
    Option<GoalStalledOn<DbInterner<'db>>>,
)>;
#[derive(Clone, Debug)]
pub(crate) struct PredicateObligation<'db, C> {
    /// The reason we have to prove this thing.
    /// FIXME: we shouldn't ignore the cause but instead change the affected visitors
    /// to only visit predicates manually.
    pub cause: C,
    pub span: Span,

    /// The environment in which we should prove this thing.
    pub param_env: ParamEnv<'db>,

    /// The thing we are trying to prove.
    pub predicate: Predicate<'db>,

    /// If we started proving this as a result of trying to prove
    /// something else, track the total depth to ensure termination.
    /// If this goes over a certain threshold, we abort compilation --
    /// in such cases, we can not say whether or not the predicate
    /// holds for certain. Stupid halting problem; such a drag.
    pub recursion_depth: usize,
}
impl<'db, C> PredicateObligation<'db, C> {
    pub(crate) fn as_goal(&self) -> Goal<DbInterner<'db>, Predicate<'db>> {
        Goal {
            param_env: self.param_env,
            predicate: self.predicate,
        }
    }
}
struct ObligationStorage<'db, C> {
    pending: PendingObligations<'db, C>,
}
impl<'db, C> Default for ObligationStorage<'db, C> {
    fn default() -> Self {
        Self {
            pending: ThinVec::new(),
        }
    }
}
impl<'db, C: Clone> ObligationStorage<'db, C> {
    pub(crate) fn register(
        &mut self,
        obligation: PredicateObligation<'db, C>,
        stalled_on: Option<GoalStalledOn<DbInterner<'db>>>,
    ) {
        self.pending.push((obligation, stalled_on));
    }
    pub(crate) fn has_pending_obligations(&self) -> bool {
        !self.pending.is_empty()
    }
    pub(crate) fn clone_pending(&self) -> PredicateObligations<'db, C> {
        self.pending.iter().map(|(o, _)| o.clone()).collect()
    }
    pub(crate) fn clone_pending_filtered<F>(&self, f: F) -> PredicateObligations<'db, C>
    where
        F: FnMut(
            &&(
                PredicateObligation<'db, C>,
                Option<GoalStalledOn<DbInterner<'db>>>,
            ),
        ) -> bool,
    {
        self.pending
            .iter()
            .filter(f)
            .map(|(o, _)| o.clone())
            .collect()
    }
}

/// Non-diagnostic outcomes retain actual original obligations. A residual's
/// stalled state carries upstream MaybeInfo, including search overflow. None
/// means it was registered but not evaluated, not a proof of no solution.
#[derive(Clone, Debug)]
pub(crate) enum NextSolverError<'db, C> {
    TrueError(PredicateObligation<'db, C>),
    Ambiguity {
        obligation: PredicateObligation<'db, C>,
        stalled_on: Option<GoalStalledOn<DbInterner<'db>>>,
    },
}
/// rustc's fatal fulfillment-progress overflow, rather than NoSolution.
/// Earlier no-solution errors and every unprocessed root remain available.
#[derive(Clone, Debug)]
pub(crate) struct FulfillmentOverflow<'db, C> {
    pub(crate) obligation: PredicateObligation<'db, C>,
    pub(crate) errors: ThinVec<NextSolverError<'db, C>>,
}
/// Pinned root fulfillment on one actual inference owner in one snapshot.
/// Caller C retains source/effect identity; it contributes no solving policy.
pub(crate) struct FulfillmentContext<'a, 'db, C> {
    owner: &'a SolverContext<'db>,
    obligations: ObligationStorage<'db, C>,
    usable_in_snapshot: usize,
    overflow: Option<FulfillmentOverflow<'db, C>>,
}
impl<'a, 'db, C: Clone> FulfillmentContext<'a, 'db, C> {
    pub(crate) fn new(owner: &'a SolverContext<'db>) -> Self {
        owner.require_source_root();
        assert!(
            owner.next_trait_solver(),
            "next-solver fulfillment requires the next solver"
        );
        Self {
            owner,
            obligations: ObligationStorage::default(),
            usable_in_snapshot: owner.num_open_snapshots(),
            overflow: None,
        }
    }
    pub(crate) fn try_evaluate_obligations(
        &mut self,
    ) -> Result<ThinVec<NextSolverError<'db, C>>, FulfillmentOverflow<'db, C>> {
        self.try_evaluate_obligations_with_inspector(|_, _| {})
    }
    /// Like rustc's collect_remaining_errors_impl, drain pending obligations as
    /// ambiguity. Preserve the actual stall rather than invoking diagnostics.
    /// Fatal overflow cannot be drained into an ordinary ambiguity/success.
    pub(crate) fn collect_remaining_errors(
        &mut self,
    ) -> Result<ThinVec<NextSolverError<'db, C>>, FulfillmentOverflow<'db, C>> {
        assert_eq!(self.usable_in_snapshot, self.owner.num_open_snapshots());
        if let Some(overflow) = &self.overflow {
            return Err(overflow.clone());
        }
        Ok(self
            .obligations
            .pending
            .drain(..)
            .map(|(obligation, stalled_on)| NextSolverError::Ambiguity {
                obligation,
                stalled_on,
            })
            .collect())
    }
    pub(crate) fn register_predicate_obligation(
        &mut self,
        obligation: PredicateObligation<'db, C>,
    ) {
        assert!(
            self.overflow.is_none(),
            "fulfillment cannot resume after fatal overflow"
        );
        let infcx = self.owner;
        assert_eq!(self.usable_in_snapshot, infcx.num_open_snapshots());

        let delegate = infcx;
        if let Some(GoalEvaluation {
            goal: _,
            certainty,
            has_changed: _,
            stalled_on,
        }) = compute_goal_fast_path(delegate, obligation.as_goal(), obligation.span)
        {
            // If we can take the fast path, don't even bother adding the goal to obligations,
            // or if `Certainty::Maybe`, add it with precise stalled_on information.
            match certainty {
                Certainty::Yes => {}
                Certainty::Maybe(_) => {
                    self.obligations.register(obligation, stalled_on);
                }
            }
        } else {
            self.obligations.register(obligation, None);
        }
    }
    pub(crate) fn try_evaluate_obligations_with_inspector(
        &mut self,
        mut inspect: impl FnMut(
            &PredicateObligation<'db, C>,
            &Result<GoalEvaluation<DbInterner<'db>>, NoSolution>,
        ),
    ) -> Result<ThinVec<NextSolverError<'db, C>>, FulfillmentOverflow<'db, C>> {
        let infcx = self.owner;
        assert_eq!(self.usable_in_snapshot, infcx.num_open_snapshots());
        if let Some(overflow) = &self.overflow {
            return Err(overflow.clone());
        }
        let mut errors = ThinVec::new();
        let mut overflow_obligation = None;
        let delegate = infcx;
        loop {
            let mut any_changed = false;

            self.obligations
                .pending
                .retain_mut(|(obligation, opt_stalled_on)| {
                    // rustc emits a diverging diagnostic at this boundary. Retain
                    // every later root without evaluating it after the typed abort.
                    if overflow_obligation.is_some() {
                        return true;
                    }
                    // Common case: still stalled; keep the obligation. This path is extremely hot in
                    // some cases; there can be thousands of pending obligations.
                    if let Some(stalled_on) = opt_stalled_on
                        && delegate.goal_remains_stalled(stalled_on)
                    {
                        return true;
                    }

                    let result = delegate.evaluate_root_goal(
                        obligation.as_goal(),
                        obligation.span,
                        opt_stalled_on.take(),
                    );
                    inspect(obligation, &result);
                    let GoalEvaluation {
                        goal,
                        certainty,
                        has_changed,
                        stalled_on,
                    } = match result {
                        Ok(result) => result,
                        Err(NoSolution) => {
                            errors.push(NextSolverError::TrueError(obligation.clone()));
                            return false;
                        }
                    };

                    // We've resolved the goal in `evaluate_root_goal`, avoid redoing this work
                    // in the next iteration. This does not resolve the inference variables
                    // constrained by evaluating the goal.
                    obligation.predicate = goal.predicate;
                    if has_changed == HasChanged::Yes {
                        if obligation.recursion_depth > infcx.interner.recursion_limit() {
                            // We limit the total count of inference progress to avoid hang so we don't
                            // try to recover from this.
                            // It's more complicated to collect all overflows thus we stopped doing that.
                            // Eager aborting is also what the old solver does.
                            //
                            // Note: it's incredibly rare to actually encounter fulfillment overflow
                            // as a single obligation would have to result in different inference progress
                            // a `recursion_depth` number of times. This mostly happens in bugs or with
                            // `Subtype` obligations because we no longer use the `sub_unification_table`
                            // in generalization.
                            overflow_obligation = Some(obligation.clone());
                            return true;
                        } else {
                            // We increment the recursion depth here to track the number of times
                            // this goal has resulted in inference progress. This doesn't precisely
                            // model the way that we track recursion depth in the old solver due
                            // to the fact that we only process root obligations, but it is a good
                            // approximation and should only result in fulfillment overflow in
                            // pathological cases.
                            obligation.recursion_depth += 1;
                            any_changed = true;
                        }
                    }

                    match certainty {
                        Certainty::Yes => false,
                        Certainty::Maybe(_) => {
                            // Update `opt_stalled_on` goal, for the next retain_mut, because we are
                            // running until a fixpoint.
                            *opt_stalled_on = stalled_on;
                            true
                        }
                    }
                });

            if let Some(obligation) = overflow_obligation {
                let overflow = FulfillmentOverflow { obligation, errors };
                self.overflow = Some(overflow.clone());
                return Err(overflow);
            }
            if !any_changed {
                break;
            }
        }

        Ok(errors)
    }
    pub(crate) fn has_pending_obligations(&self) -> bool {
        self.obligations.has_pending_obligations()
    }
    pub(crate) fn pending_obligations(&self) -> PredicateObligations<'db, C> {
        self.obligations.clone_pending()
    }
    pub(crate) fn pending_obligations_potentially_referencing_sub_root(
        &self,
        vid: ty::TyVid,
    ) -> PredicateObligations<'db, C> {
        let infcx = self.owner;
        // `-Zdisable-fast-paths`: same gate as the other new-solver fast paths.
        if infcx.disable_trait_solver_fast_paths() {
            return self.obligations.clone_pending();
        }
        self.obligations.clone_pending_filtered(|(_, stalled_on)| {
            let Some(stalled_on) = stalled_on else {
                return true;
            };
            // Don't reuse the sub-unification roots cached on `stalled_on`:
            // a later sub-unification merge can have changed which root
            // each stalled var belongs to, so the cached info can be stale.
            // Walk `stalled_vars` and recompute the current root instead.
            //
            // Conservative here: if a stalled var no longer resolves to an
            // infer var, some unification happened, so the goal is no longer
            // stalled. Include it to be re-evaluated downstream.
            stalled_on
                .stalled_vars
                .iter()
                .filter_map(|arg| arg.as_type(infcx.interner))
                .any(|ty| match infcx.shallow_resolve(ty).kind() {
                    ty::Infer(ty::TyVar(tv)) => infcx.sub_unification_table_root_var(tv) == vid,
                    _ => true,
                })
        })
    }
    pub(crate) fn pending_obligations_potentially_referencing_float_infer(
        &self,
    ) -> PredicateObligations<'db, C> {
        let infcx = self.owner;
        // `-Zdisable-fast-paths`: same gate as the other new-solver fast paths.
        if infcx.disable_trait_solver_fast_paths() {
            return self.obligations.clone_pending();
        }

        self.obligations.clone_pending_filtered(|(_, stalled_on)| {
            let Some(stalled_on) = stalled_on else {
                return true;
            };
            // If the stalled vars don't have float infers, the nested goals won't
            // have them either. We only create float infers for user written literals.
            stalled_on
                .stalled_vars
                .iter()
                .filter_map(|arg| arg.as_type(infcx.interner))
                .any(|ty| matches!(infcx.shallow_resolve(ty).kind(), ty::Infer(ty::FloatVar(_))))
        })
    }
}

impl<'a, 'db, C: Clone> FulfillmentContext<'a, 'db, C> {
    pub(crate) fn pending_with_stall(&self) -> PendingObligations<'db, C> {
        self.obligations.pending.clone()
    }
}

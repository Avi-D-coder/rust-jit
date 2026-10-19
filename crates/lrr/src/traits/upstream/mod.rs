//! The shared solver's ordinary root-goal entry; no alternate solver.
//!
//! This is the typed integration boundary, not yet a concrete LRR Interner or
//! SolverDelegate. Source/header/ParamEnv transport, demand suspension and
//! selected-impl witness mapping must join before any runtime acceptance.
use rustc_next_trait_solver::{
    delegate::SolverDelegate,
    solve::{GoalEvaluation, SolverDelegateEvalExt},
};
use rustc_type_ir::{
    solve::{Goal, NoSolution},
    Interner,
};

mod binders;
mod constructors;
pub(crate) mod context;
pub(crate) mod facts;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod parameters;
mod predicates;
mod printing;
pub(crate) mod storage;
mod traversal;

pub(crate) const SOURCE_REVISION: &str = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18";

/// Called only for an external demanded goal; recursive child goals stay in
/// upstream's search graph. The caller retains the actual inference/dependency
/// context and maps the canonical answer; no guest work happens in this entry.
pub(crate) fn evaluate<D: SolverDelegate>(
    delegate: &D,
    goal: Goal<D::Interner, <D::Interner as Interner>::Predicate>,
    span: <D::Interner as Interner>::Span,
) -> Result<GoalEvaluation<D::Interner>, NoSolution> {
    delegate.evaluate_root_goal(goal, span, None)
}

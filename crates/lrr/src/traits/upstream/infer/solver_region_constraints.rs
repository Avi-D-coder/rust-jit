//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{DbInterner, Span};
use tracing::instrument;

pub type SolverRegionConstraint<'db> =
    rustc_type_ir::region_constraint::RegionConstraint<DbInterner<'db>, Span>;

#[derive(Clone, Debug)]
pub(crate) struct SolverRegionConstraintStorage<'db>(Option<SolverRegionConstraint<'db>>);

impl<'db> SolverRegionConstraintStorage<'db> {
    pub(crate) fn new() -> Self {
        Self(None)
    }

    pub(crate) fn get_constraint(&self) -> SolverRegionConstraint<'db> {
        match &self.0 {
            Some(v) => v.clone(),
            None => SolverRegionConstraint::new_true(),
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub(crate) fn overwrite(&mut self, constraint: SolverRegionConstraint<'db>) {
        self.0 = Some(constraint);
    }
}

use super::InferenceContext;
use super::snapshot::undo_log::UndoLog;
use ena::undo_log::UndoLogs;
use rustc_type_ir::TypeVisitableExt;
impl<'db> InferenceContext<'db> {
    pub fn get_solver_region_constraint(&self) -> SolverRegionConstraint<'db> {
        self.inner
            .borrow()
            .solver_region_constraint_storage
            .get_constraint()
    }
    pub fn overwrite_solver_region_constraint(&self, constraint: SolverRegionConstraint<'db>) {
        assert!(
            !constraint.has_escaping_bound_vars(),
            "solver region constraint has escaping bound vars, which is indicative of a bug in how constraints are handled: {constraint:?}",
        );
        let mut inner = self.inner.borrow_mut();
        let old_constraint = inner.solver_region_constraint_storage.get_constraint();
        inner
            .undo_log
            .push(UndoLog::OverwriteSolverRegionConstraint { old_constraint });
        inner.solver_region_constraint_storage.overwrite(constraint);
    }
    pub fn register_solver_region_constraint(&self, c: SolverRegionConstraint<'db>) {
        let mut inner = self.inner.borrow_mut();

        let old_constraint = inner.solver_region_constraint_storage.get_constraint();
        let new_constraint = rustc_type_ir::region_constraint::RegionConstraint::build_and(
            c,
            old_constraint.clone(),
        );

        // FIXME(-Zassumptions-on-binders): This is pretty bad for perf, we don't make incremental
        // changes to the region constraints, instead we just rewrite the entire thing every time
        // and store the old version.
        inner
            .undo_log
            .push(UndoLog::OverwriteSolverRegionConstraint { old_constraint });
        inner
            .solver_region_constraint_storage
            .overwrite(new_constraint);
    }
}

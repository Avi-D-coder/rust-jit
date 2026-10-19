//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::region_constraints::{ConstraintKind, RegionConstraintData};
use super::snapshot::undo_log::UndoLog;
use super::{
    ArgOutlivesPredicate, DbInterner, InferCtxtInner, InferenceContext, Span,
    TypeOutlivesConstraint,
};
use ena::undo_log::UndoLogs;
use rustc_type_ir::data_structures::HashMap as FxHashMap;
use rustc_type_ir::{self as ty, Interner};
use std::collections::hash_map::Entry;
use tracing::debug;
#[derive(Clone, Debug, Default)]
pub struct QueryRegionConstraints<'db> {
    pub constraints: Vec<QueryRegionConstraint<'db>>,
    pub assumptions: Vec<ArgOutlivesPredicate<'db>>,
}
#[derive(Copy, Clone, Debug)]
pub struct QueryRegionConstraint<'db> {
    pub constraint: ty::RegionConstraint<DbInterner<'db>>,
    pub origin: Span,
    pub visible_for_leak_check: ty::solve::VisibleForLeakCheck,
}
pub fn make_query_region_constraints<'db>(
    outlives_obligations: Vec<TypeOutlivesConstraint<'db>>,
    region_constraints: &RegionConstraintData<'db>,
    assumptions: Vec<ArgOutlivesPredicate<'db>>,
) -> QueryRegionConstraints<'db> {
    let RegionConstraintData {
        constraints,
        verifys,
    } = region_constraints;

    assert!(verifys.is_empty());

    debug!(?constraints);

    let constraints: Vec<QueryRegionConstraint<'db>> = constraints
        .iter()
        .map(|&(c, origin)| match c.kind {
            ConstraintKind::VarSubVar
            | ConstraintKind::RegSubVar
            | ConstraintKind::VarSubReg
            | ConstraintKind::RegSubReg => {
                // Swap regions because we are going from sub (<=) to outlives (>=).
                let constraint = ty::OutlivesClause(c.sup.into(), c.sub).into();
                QueryRegionConstraint {
                    constraint,
                    origin,
                    visible_for_leak_check: c.visible_for_leak_check,
                }
            }

            ConstraintKind::VarEqVar | ConstraintKind::VarEqReg | ConstraintKind::RegEqReg => {
                let constraint = ty::RegionEqPredicate(c.sup, c.sub).into();
                QueryRegionConstraint {
                    constraint,
                    origin,
                    visible_for_leak_check: c.visible_for_leak_check,
                }
            }
        })
        .chain(outlives_obligations.into_iter().map(
            |TypeOutlivesConstraint {
                 sub_region,
                 sup_type,
                 origin,
             }| {
                QueryRegionConstraint {
                    constraint: ty::OutlivesClause(sup_type.into(), sub_region).into(),
                    origin,
                    // We don't do leak checks for type outlives
                    visible_for_leak_check: ty::solve::VisibleForLeakCheck::Unreachable,
                }
            },
        ))
        .collect();

    QueryRegionConstraints {
        constraints,
        assumptions,
    }
}
impl<'db> InferenceContext<'db> {
    pub fn with_region_constraints<R>(
        &self,
        op: impl FnOnce(&RegionConstraintData<'db>) -> R,
    ) -> R {
        let mut inner = self.inner.borrow_mut();
        op(inner.unwrap_region_constraints().data())
    }
    pub fn register_region_assumption(&self, assumption: ArgOutlivesPredicate<'db>) {
        let mut inner = self.inner.borrow_mut();
        inner.undo_log.push(UndoLog::PushRegionAssumption);
        inner.region_assumptions.push(assumption);
    }
    pub fn take_registered_region_assumptions(&self) -> Vec<ArgOutlivesPredicate<'db>> {
        assert!(
            !self.in_snapshot(),
            "cannot take registered region assumptions in a snapshot"
        );
        std::mem::take(&mut self.inner.borrow_mut().region_assumptions)
    }
    pub fn make_deduplicated_region_constraints(
        &self,
    ) -> Vec<(
        ty::RegionConstraint<DbInterner<'db>>,
        ty::solve::VisibleForLeakCheck,
    )> {
        // Cannot use `take_registered_region_obligations` as we may compute the response
        // inside of a `probe` whenever we have multiple choices inside of the solver.
        let region_obligations = self.inner.borrow().region_obligations().to_owned();
        let region_assumptions = self.inner.borrow().region_assumptions().to_owned();
        let region_constraints = self.with_region_constraints(|region_constraints| {
            make_query_region_constraints(
                region_obligations,
                region_constraints,
                region_assumptions,
            )
        });

        let mut seen = FxHashMap::default();
        let mut constraints = vec![];
        for QueryRegionConstraint {
            constraint: outlives,
            visible_for_leak_check: vis,
            ..
        } in region_constraints.constraints
        {
            match seen.entry(outlives) {
                Entry::Occupied(occupied) => {
                    let idx = occupied.get();
                    let (_, prev_vis): &mut (_, ty::solve::VisibleForLeakCheck) =
                        constraints.get_mut(*idx).unwrap();
                    *prev_vis = (*prev_vis).or(vis);
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(constraints.len());
                    constraints.push((outlives, vis));
                }
            }
        }
        constraints
    }
}

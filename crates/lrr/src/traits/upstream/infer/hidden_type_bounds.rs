//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Ty};
use crate::traits::upstream::{identity::DefinitionId, model};
use rustc_type_ir::{
    self as ty, InferCtxtLike, Interner, TypeFoldable, TypeFolder, TypeSuperFoldable,
    TypeVisitableExt, Unnormalized, inherent::*,
};
use tracing::debug;
type Goal<'db, P> = ty::solve::Goal<DbInterner<'db>, P>;
type Clause<'db> = model::Clause<'db, PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db, PreparedProvider<'db>>;
pub struct BottomUpFolder<'db, F, G, H>
where
    F: FnMut(Ty<'db>) -> Ty<'db>,
    G: FnMut(ty::Region<DbInterner<'db>>) -> ty::Region<DbInterner<'db>>,
    H: FnMut(Const<'db>) -> Const<'db>,
{
    pub interner: DbInterner<'db>,
    pub ty_op: F,
    pub lt_op: G,
    pub ct_op: H,
}

impl<'db, F, G, H> TypeFolder<DbInterner<'db>> for BottomUpFolder<'db, F, G, H>
where
    F: FnMut(Ty<'db>) -> Ty<'db>,
    G: FnMut(ty::Region<DbInterner<'db>>) -> ty::Region<DbInterner<'db>>,
    H: FnMut(Const<'db>) -> Const<'db>,
{
    fn cx(&self) -> DbInterner<'db> {
        self.interner
    }

    fn fold_ty(&mut self, ty: Ty<'db>) -> Ty<'db> {
        let t = ty.super_fold_with(self);
        (self.ty_op)(t)
    }

    fn fold_region(&mut self, r: ty::Region<DbInterner<'db>>) -> ty::Region<DbInterner<'db>> {
        // This one is a little different, because `super_fold_with` is not
        // implemented on non-recursive `Region`.
        (self.lt_op)(r)
    }

    fn fold_const(&mut self, ct: Const<'db>) -> Const<'db> {
        let ct = ct.super_fold_with(self);
        (self.ct_op)(ct)
    }
}
impl<'db> InferenceContext<'db> {
    pub fn add_item_bounds_for_hidden_type(
        &self,
        def_id: DefinitionId,
        args: crate::traits::upstream::model::Args<'db, PreparedProvider<'db>>,
        param_env: model::ParamEnv<'db, PreparedProvider<'db>>,
        hidden_ty: Ty<'db>,
        goals: &mut Vec<Goal<'db, Predicate<'db>>>,
    ) {
        let tcx = self.interner;
        // Require that the hidden type is well-formed. We have to
        // make sure we wf-check the hidden type to fix #114728.
        //
        // However, we don't check that all types are well-formed.
        // We only do so for types provided by the user or if they are
        // "used", e.g. for method selection.
        //
        // This means we never check the wf requirements of the hidden
        // type during MIR borrowck, causing us to infer the wrong
        // lifetime for its member constraints which then results in
        // unexpected region errors.
        goals.push(Goal::new(
            tcx,
            param_env,
            ty::ClauseKind::WellFormed(hidden_ty.into()),
        ));

        let replace_opaques_in = |clause: Clause<'db>, goals: &mut Vec<_>| {
            clause.fold_with(&mut BottomUpFolder {
                interner: tcx,
                ty_op: |ty| match ty.kind() {
                    // We can't normalize associated types from `rustc_infer`,
                    // but we can eagerly register inference variables for them.
                    // FIXME(RPITIT): Don't replace RPITITs with inference vars.
                    // FIXME(inherent_associated_types): Extend this to support `ty::Inherent`, too.
                    ty::Alias(
                        _,
                        projection_ty @ ty::AliasTy {
                            kind: ty::Projection { def_id },
                            ..
                        },
                    ) if !projection_ty.has_escaping_bound_vars()
                        && !tcx.is_impl_trait_in_trait(def_id)
                        && !self.next_trait_solver() =>
                    {
                        let ty_var = self.next_ty_var(self.interner.def_span(def_id));
                        goals.push(Goal::new(
                            self.interner,
                            param_env,
                            ty::ProjectionClause {
                                projection_term: projection_ty.into(),
                                term: ty_var.into(),
                            },
                        ));
                        ty_var
                    }
                    // Replace all other mentions of the same opaque type with the hidden type,
                    // as the bounds must hold on the hidden type after all.
                    ty::Alias(
                        _,
                        ty::AliasTy {
                            kind: ty::Opaque { def_id: def_id2 },
                            args: args2,
                            ..
                        },
                    ) if def_id == def_id2 && args == args2 => hidden_ty,
                    _ => ty,
                },
                lt_op: |lt| lt,
                ct_op: |ct| ct,
            })
        };

        let item_bounds = tcx.explicit_item_bounds(def_id);
        for (predicate, _) in item_bounds
            .iter_instantiated(tcx, args)
            .map(Unnormalized::skip_norm_wip)
        {
            let predicate = replace_opaques_in(predicate, goals);

            // Require that the predicate holds for the concrete type.
            debug!(?predicate);
            goals.push(Goal::new(self.interner, param_env, predicate));
        }

        // If this opaque is being defined and it's conditionally const,
        if self.interner.is_conditionally_const(def_id) {
            let item_bounds = tcx.explicit_implied_const_bounds(def_id);
            for predicate in item_bounds
                .iter_instantiated(tcx, args)
                .map(Unnormalized::skip_norm_wip)
            {
                let predicate = replace_opaques_in(
                    predicate.to_host_effect_clause(self.interner, ty::BoundConstness::Maybe),
                    goals,
                );

                // Require that the predicate holds for the concrete type.
                debug!(?predicate);
                goals.push(Goal::new(self.interner, param_env, predicate));
            }
        }
    }
}

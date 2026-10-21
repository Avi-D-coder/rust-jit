//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Span, Ty};
use crate::traits::upstream::model::{self, Arg};
use rustc_type_ir::data_structures::HashSet as FxHashSet;
use rustc_type_ir::inherent::{GenericArg as _, ParamEnv as _, Term as _, Ty as _};
use rustc_type_ir::lang_items::SolverTraitLangItem;
use rustc_type_ir::solve::{
    GoalStalledOn, GoalStalledOnOpaques, MaybeInfo, SizedTraitKind, SucceededInErased,
    TyOrConstInferVar,
};
use rustc_type_ir::{
    self as ty, InferCtxtLike, Interner, TermKind, TypeFlags, TypeSuperVisitable, TypeVisitable,
    TypeVisitableExt, TypeVisitor, inherent::*,
};
use thin_vec::{ThinVec, thin_vec};
use tracing::debug;
type Goal<'db, P> = ty::solve::Goal<DbInterner<'db>, P>;
type Predicate<'db> = model::Predicate<'db, PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type GenericArg<'db> = Arg<'db, PreparedProvider<'db>>;
type Term<'db> = model::Term<'db, PreparedProvider<'db>>;
type ComputeGoalFastPathOutcome<'db> = ty::solve::ComputeGoalFastPathOutcome<DbInterner<'db>>;
fn goal_stalled_on_args<'db>(
    stalled_vars: ThinVec<TyOrConstInferVar>,
) -> ComputeGoalFastPathOutcome<'db> {
    ComputeGoalFastPathOutcome::TriviallyStalled {
        stalled_on: GoalStalledOn {
            stalled_vars,
            sub_roots: ThinVec::new(),
            stalled_maybe_info: MaybeInfo::AMBIGUOUS,
            opaques: GoalStalledOnOpaques::No,
        },
    }
}

/// Create a [`ComputeGoalFastPathOutcome`] signalling the  goal is stalled
/// on a list of [`ty::GenericArg`] *or* the opaque type storage being nonempty.
///
fn goal_stalled_on_args_or_nonempty_opaques<'db>(
    stalled_vars: ThinVec<TyOrConstInferVar>,
) -> ComputeGoalFastPathOutcome<'db> {
    ComputeGoalFastPathOutcome::TriviallyStalled {
        stalled_on: GoalStalledOn {
            stalled_vars,
            sub_roots: ThinVec::new(),
            stalled_maybe_info: MaybeInfo::AMBIGUOUS,
            opaques: GoalStalledOnOpaques::Yes {
                num_opaques_in_storage: 0,
                // This function should only be called when not in erased mode,
                // otherwise this is wrong. The `compute_goal_fast_path` does this
                // through `known_no_opaque_types_in_storage`
                previously_succeeded_in_erased: SucceededInErased::No,
            },
        },
    }
}

struct CollectNonRegionInfer<'db> {
    infers: ThinVec<GenericArg<'db>>,
    visited: FxHashSet<Ty<'db>>,
}

impl<'db> TypeVisitor<DbInterner<'db>> for CollectNonRegionInfer<'db> {
    type Result = ();
    fn visit_ty(&mut self, ty: Ty<'db>) {
        if self.visited.contains(&ty) {
            return;
        }

        match ty.kind() {
            ty::Infer(_) => self.infers.push(ty.into()),
            _ => ty.super_visit_with(self),
        }

        self.visited.insert(ty);
    }

    fn visit_const(&mut self, ct: Const<'db>) {
        match ct.kind() {
            ty::ConstKind::Infer(_) => self.infers.push(ct.into()),
            _ => ct.super_visit_with(self),
        }
    }
}

impl<'db> InferenceContext<'db> {
    pub fn known_no_opaque_types_in_storage(&self) -> bool {
        self.inner.borrow_mut().opaque_types().is_empty()
            // in erased mode, observing that opaques are empty aren't enough to give a result
            // here, so let's try the slow path instead.
            && !self.typing_mode_raw().is_erased_not_coherence()
    }
    pub fn compute_goal_fast_path(
        &self,
        goal: Goal<'db, Predicate<'db>>,
        span: Span,
    ) -> ComputeGoalFastPathOutcome<'db> {
        self.require_source_root();
        use ComputeGoalFastPathOutcome as Outcome;

        // FIXME(-Zassumptions-on-binders): actually handle fast path
        if self.interner.assumptions_on_binders() {
            return Outcome::NoFastPath;
        }

        let pred = goal.predicate.kind();
        match pred.skip_binder() {
            ty::PredicateKind::Clause(ty::ClauseKind::Trait(trait_pred)) => {
                let trait_pred = pred.rebind(trait_pred);

                let self_ty = self.shallow_resolve(trait_pred.self_ty().skip_binder());
                if let Some(vid) = self_ty.ty_vid()
                // We don't do this fast path when opaques are defined since we may
                // eventually use opaques to incompletely guide inference via ty var
                // self types.
                // FIXME: Properly consider opaques here.
                && self.known_no_opaque_types_in_storage()
                {
                    goal_stalled_on_args_or_nonempty_opaques(thin_vec![TyOrConstInferVar::Ty(vid)])
                } else if trait_pred.polarity() == ty::ClausePolarity::Positive {
                    match self.interner.as_trait_lang_item(trait_pred.def_id()) {
                        Some(SolverTraitLangItem::Sized) | Some(SolverTraitLangItem::MetaSized) => {
                            let predicate = self.deeply_resolve_ignoring_regions(goal.predicate);
                            if sizedness_fast_path(self.interner, predicate, goal.param_env) {
                                Outcome::TriviallyHolds
                            } else {
                                Outcome::NoFastPath
                            }
                        }
                        Some(SolverTraitLangItem::Copy | SolverTraitLangItem::Clone) => {
                            let self_ty = self.deeply_resolve_ignoring_regions(
                                trait_pred.self_ty().skip_binder(),
                            );
                            // Unlike `Sized` traits, which always prefer the built-in impl,
                            // `Copy`/`Clone` may be shadowed by a param-env candidate which
                            // could force a lifetime error or guide inference. While that's
                            // not generally desirable, it is observable, so for now let's
                            // ignore this fast path for types that have regions or infer.
                            if !self_ty
                                .has_type_flags(TypeFlags::HAS_FREE_REGIONS | TypeFlags::HAS_INFER)
                                && self_ty.is_trivially_pure_clone_copy()
                            {
                                Outcome::TriviallyHolds
                            } else {
                                Outcome::NoFastPath
                            }
                        }
                        _ => Outcome::NoFastPath,
                    }
                } else {
                    Outcome::NoFastPath
                }
            }
            ty::PredicateKind::DynCompatible(def_id)
                if self.interner.trait_is_dyn_compatible(def_id) =>
            {
                Outcome::TriviallyHolds
            }
            ty::PredicateKind::Clause(ty::ClauseKind::RegionOutlives(outlives)) => {
                if outlives.has_escaping_bound_vars() {
                    return Outcome::NoFastPath;
                }

                // We use `VisibleForLeakCheck::Yes` for `RegionOutlives` goals without
                // the fast path. However, we always change region constraints from nested
                // goals to `VisibleForLeakCheck::No`, so to be consistent with this
                // behavior as we avoid the nested goal boundary, we have to manually
                // set the flag.
                self.sub_regions(
                    outlives.1,
                    outlives.0,
                    ty::solve::VisibleForLeakCheck::No,
                    span,
                );
                Outcome::TriviallyHolds
            }
            ty::PredicateKind::Clause(ty::ClauseKind::TypeOutlives(outlives)) => {
                if outlives.has_escaping_bound_vars() {
                    return Outcome::NoFastPath;
                }

                let ty = self.deeply_resolve_ignoring_regions(outlives.0);
                if ty.has_non_rigid_aliases() {
                    return Outcome::NoFastPath;
                }

                let mut infer_collector = CollectNonRegionInfer {
                    infers: Default::default(),
                    visited: Default::default(),
                };
                ty.visit_with(&mut infer_collector);
                let infers = infer_collector.infers;
                if !infers.is_empty() {
                    return goal_stalled_on_args(
                        infers
                            .into_iter()
                            .map(|i| {
                                TyOrConstInferVar::maybe_from_generic_arg::<DbInterner<'db>>(i)
                                    .unwrap()
                            })
                            .collect(),
                    );
                }

                self.register_type_outlives_constraint(outlives.0, outlives.1, span);

                Outcome::TriviallyHolds
            }
            ty::PredicateKind::Subtype(ty::SubtypePredicate { a, b, .. })
            | ty::PredicateKind::Coerce(ty::CoercePredicate { a, b }) => {
                if a.has_escaping_bound_vars() || b.has_escaping_bound_vars() {
                    return Outcome::NoFastPath;
                }

                match (
                    self.shallow_resolve(a).kind(),
                    self.shallow_resolve(b).kind(),
                ) {
                    (ty::Infer(ty::TyVar(a_vid)), ty::Infer(ty::TyVar(b_vid))) => {
                        self.sub_unify_ty_vids_raw(a_vid, b_vid);
                        goal_stalled_on_args(thin_vec![
                            TyOrConstInferVar::Ty(a_vid),
                            TyOrConstInferVar::Ty(b_vid),
                        ])
                    }
                    _ => Outcome::NoFastPath,
                }
            }
            ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(ct, _)) => {
                if ct.has_escaping_bound_vars() {
                    return Outcome::NoFastPath;
                }

                let arg = self.shallow_resolve_const(ct);
                if let Some(vid) = arg.ct_vid() {
                    goal_stalled_on_args(thin_vec![TyOrConstInferVar::Const(vid)])
                } else {
                    Outcome::NoFastPath
                }
            }
            ty::PredicateKind::Clause(ty::ClauseKind::WellFormed(arg)) => {
                if arg.has_escaping_bound_vars() {
                    return Outcome::NoFastPath;
                }

                let arg = self.shallow_resolve_term(arg);
                if arg.is_trivially_wf(self.interner) {
                    Outcome::TriviallyHolds
                } else if arg.is_infer() {
                    goal_stalled_on_args(thin_vec![
                        TyOrConstInferVar::maybe_from_term::<DbInterner<'db>>(arg)
                            .expect("its an infer var"),
                    ])
                } else {
                    Outcome::NoFastPath
                }
            }
            _ => Outcome::NoFastPath,
        }
    }
}
pub fn sizedness_fast_path<'db>(
    tcx: DbInterner<'db>,
    predicate: Predicate<'db>,
    param_env: ParamEnv<'db>,
) -> bool {
    // Proving `Sized`/`MetaSized`, very often on "obviously sized" types like
    // `&T`, accounts for about 60% percentage of the predicates we have to prove. No need to
    // canonicalize and all that for such cases.
    if let ty::PredicateKind::Clause(ty::ClauseKind::Trait(trait_pred)) =
        predicate.kind().skip_binder()
        && trait_pred.polarity == ty::ClausePolarity::Positive
    {
        let sizedness = match tcx.as_trait_lang_item(trait_pred.def_id()) {
            Some(SolverTraitLangItem::Sized) => SizedTraitKind::Sized,
            Some(SolverTraitLangItem::MetaSized) => SizedTraitKind::MetaSized,
            _ => return false,
        };

        if trait_pred.self_ty().has_trivial_sizedness(tcx, sizedness) {
            debug!("fast path -- trivial sizedness");
            return true;
        }

        if matches!(
            trait_pred.self_ty().kind(),
            ty::Param(_) | ty::Placeholder(_)
        ) {
            for clause in param_env.caller_bounds() {
                if let ty::ClauseKind::Trait(clause_pred) = clause.kind().skip_binder()
                    && clause_pred.polarity == ty::ClausePolarity::Positive
                    && clause_pred.self_ty() == trait_pred.self_ty()
                    && (clause_pred.def_id() == trait_pred.def_id()
                        || (sizedness == SizedTraitKind::MetaSized
                            && tcx.is_trait_lang_item(
                                clause_pred.def_id(),
                                SolverTraitLangItem::Sized,
                            )))
                {
                    return true;
                }
            }
        }
    }

    false
}
impl<'db> Ty<'db> {
    pub fn ty_vid(self) -> Option<ty::TyVid> {
        match self.kind() {
            ty::Infer(ty::TyVar(vid)) => Some(vid),
            _ => None,
        }
    }
    pub fn has_trivial_sizedness(self, tcx: DbInterner<'db>, sizedness: SizedTraitKind) -> bool {
        match self.kind() {
            ty::Infer(ty::IntVar(_) | ty::FloatVar(_))
            | ty::Uint(_)
            | ty::Int(_)
            | ty::Bool
            | ty::Float(_)
            | ty::FnDef(..)
            | ty::FnPtr(..)
            | ty::UnsafeBinder(_)
            | ty::RawPtr(..)
            | ty::Char
            | ty::Ref(..)
            | ty::Coroutine(..)
            | ty::CoroutineWitness(..)
            | ty::Array(..)
            | ty::Pat(..)
            | ty::Closure(..)
            | ty::CoroutineClosure(..)
            | ty::Never
            | ty::Error(_) => true,

            ty::Str | ty::Slice(_) | ty::Dynamic(_, _) => match sizedness {
                SizedTraitKind::Sized => false,
                SizedTraitKind::MetaSized => true,
            },

            ty::Foreign(..) => match sizedness {
                SizedTraitKind::Sized | SizedTraitKind::MetaSized => false,
            },

            ty::Tuple(tys) => tys
                .last()
                .is_none_or(|ty| ty.has_trivial_sizedness(tcx, sizedness)),

            ty::Adt(def, args) => def.sizedness_constraint(tcx, sizedness).is_none_or(|ty| {
                ty.instantiate(tcx, args)
                    .skip_norm_wip()
                    .has_trivial_sizedness(tcx, sizedness)
            }),

            ty::Alias(..) | ty::Param(_) | ty::Placeholder(..) | ty::Bound(..) => false,

            ty::Infer(ty::TyVar(_)) => false,

            ty::Infer(ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_)) => {
                panic!(
                    "`has_trivial_sizedness` applied to unexpected type: {:?}",
                    self
                )
            }
        }
    }
    pub fn is_trivially_pure_clone_copy(self) -> bool {
        match self.kind() {
            ty::Bool | ty::Char | ty::Never => true,

            // These aren't even `Clone`
            ty::Str | ty::Slice(..) | ty::Foreign(..) | ty::Dynamic(..) => false,

            ty::Infer(ty::InferTy::FloatVar(_) | ty::InferTy::IntVar(_))
            | ty::Int(..)
            | ty::Uint(..)
            | ty::Float(..) => true,

            // ZST which can't be named are fine.
            ty::FnDef(..) => true,

            ty::Array(element_ty, _len) => element_ty.is_trivially_pure_clone_copy(),

            // A 100-tuple isn't "trivial", so doing this only for reasonable sizes.
            ty::Tuple(field_tys) => {
                field_tys.len() <= 3 && field_tys.iter().all(Self::is_trivially_pure_clone_copy)
            }

            ty::Pat(ty, _) => ty.is_trivially_pure_clone_copy(),

            // Sometimes traits aren't implemented for every ABI or arity,
            // because we can't be generic over everything yet.
            ty::FnPtr(..) => false,

            // Definitely absolutely not copy.
            ty::Ref(_, _, ty::Mutability::Mut) => false,

            // The standard library has a blanket Copy impl for shared references and raw pointers,
            // for all unsized types.
            ty::Ref(_, _, ty::Mutability::Not) | ty::RawPtr(..) => true,

            ty::Coroutine(..) | ty::CoroutineWitness(..) => false,

            // Might be, but not "trivial" so just giving the safe answer.
            ty::Adt(..) | ty::Closure(..) | ty::CoroutineClosure(..) => false,

            ty::UnsafeBinder(_) => false,

            // Needs normalization or revealing to determine, so no is the safe answer.
            ty::Alias(..) => false,

            ty::Param(..) | ty::Placeholder(..) | ty::Bound(..) | ty::Infer(..) | ty::Error(..) => {
                false
            }
        }
    }
    pub fn is_trivially_wf(self, tcx: DbInterner<'db>) -> bool {
        match self.kind() {
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Str
            | ty::Never
            | ty::Param(_)
            | ty::Placeholder(_)
            | ty::Bound(..) => true,

            ty::Slice(ty) => {
                ty.is_trivially_wf(tcx) && ty.has_trivial_sizedness(tcx, SizedTraitKind::Sized)
            }
            ty::RawPtr(ty, _) => ty.is_trivially_wf(tcx),

            ty::FnPtr(sig_tys, _) => sig_tys
                .skip_binder()
                .inputs_and_output
                .iter()
                .all(|ty| ty.is_trivially_wf(tcx)),
            ty::Ref(_, ty, _) => ty.is_global() && ty.is_trivially_wf(tcx),

            ty::Infer(infer) => match infer {
                ty::TyVar(_) => false,
                ty::IntVar(_) | ty::FloatVar(_) => true,
                ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_) => true,
            },

            ty::Adt(_, _)
            | ty::Tuple(_)
            | ty::Array(..)
            | ty::Foreign(_)
            | ty::Pat(_, _)
            | ty::FnDef(..)
            | ty::UnsafeBinder(..)
            | ty::Dynamic(..)
            | ty::Closure(..)
            | ty::CoroutineClosure(..)
            | ty::Coroutine(..)
            | ty::CoroutineWitness(..)
            | ty::Alias(..)
            | ty::Error(_) => false,
        }
    }
}
impl<'db> Term<'db> {
    pub fn is_trivially_wf(&self, tcx: DbInterner<'db>) -> bool {
        match self.kind() {
            TermKind::Ty(ty) => ty.is_trivially_wf(tcx),
            TermKind::Const(ct) => ct.is_trivially_wf(),
        }
    }
}

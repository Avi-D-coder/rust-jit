//! Pinned rustc unnormalized WF goals; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_wellformed.py.
use super::{Const, DbInterner, InferenceContext, Span, Ty};
use crate::traits::upstream::facts::SourceFacts as _;
use crate::traits::upstream::{
    identity::DefinitionId as DefId,
    model::{self, *},
};
use rustc_type_ir::data_structures::DelayedSet;
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::lang_items::SolverTraitLangItem;
use rustc_type_ir::solve::Goal;
use rustc_type_ir::{
    self as ty, Interner, PredicateProxy, TermKind, TypeSuperVisitable, TypeVisitable,
    TypeVisitableExt, TypeVisitor, Upcast, inherent::*,
};
use tracing::{debug, instrument};
type ParamEnv<'db> = model::ParamEnv<'db, super::PreparedProvider<'db>>;
type Args<'db> = model::Args<'db, super::PreparedProvider<'db>>;
type Term<'db> = model::Term<'db, super::PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db, super::PreparedProvider<'db>>;
#[derive(Debug)]
pub(super) struct Obligation<'db> {
    pub(super) predicate: Predicate<'db>,
    param_env: model::ParamEnv<'db, super::PreparedProvider<'db>>,
}
impl<'db> Obligation<'db> {
    fn with_depth<P: Upcast<DbInterner<'db>, Predicate<'db>>>(
        cx: DbInterner<'db>,
        _cause: (),
        _depth: usize,
        param_env: model::ParamEnv<'db, super::PreparedProvider<'db>>,
        predicate: P,
    ) -> Self {
        Self {
            param_env,
            predicate: predicate.upcast(cx),
        }
    }
    fn new<P: Upcast<DbInterner<'db>, Predicate<'db>>>(
        cx: DbInterner<'db>,
        cause: (),
        param_env: model::ParamEnv<'db, super::PreparedProvider<'db>>,
        predicate: P,
    ) -> Self {
        Self::with_depth(cx, cause, 0, param_env, predicate)
    }
    pub(super) fn as_goal(self) -> Goal<DbInterner<'db>, Predicate<'db>> {
        Goal {
            param_env: self.param_env,
            predicate: self.predicate,
        }
    }
    fn from_goal(goal: Goal<DbInterner<'db>, Predicate<'db>>) -> Self {
        Self {
            param_env: goal.param_env,
            predicate: goal.predicate,
        }
    }
}
struct SelectionContext<'a, 'db> {
    infcx: &'a InferenceContext<'db>,
}
impl<'a, 'db> SelectionContext<'a, 'db> {
    fn new(infcx: &'a InferenceContext<'db>) -> Self {
        Self { infcx }
    }
    fn tcx(&self) -> DbInterner<'db> {
        self.infcx.interner
    }
}
fn dummy_span<'db>() -> Span {
    <Span as ty::inherent::Span<DbInterner<'db>>>::dummy()
}
enum WfLangItem {
    Sized,
    ConstParamTy,
}
fn is_wf_lang_item(cx: DbInterner<'_>, id: DefId, item: WfLangItem) -> bool {
    match item {
        WfLangItem::Sized => cx
            .services()
            .is_trait_lang_item(cx, id, SolverTraitLangItem::Sized),
        WfLangItem::ConstParamTy => cx.services().const_param_ty_trait() == Some(id),
    }
}
struct WfPredicates<'a, 'db> {
    infcx: &'a InferenceContext<'db>,
    param_env: model::ParamEnv<'db, super::PreparedProvider<'db>>,
    span: Span,
    out: Vec<Obligation<'db>>,
    recursion_depth: usize,
    visited_tys: DelayedSet<Ty<'db>>,
}
impl<'a, 'db> WfPredicates<'a, 'db> {
    fn cause(&self, _code: ()) -> () {}
}
pub(super) fn unnormalized_obligations<'db>(
    infcx: &InferenceContext<'db>,
    param_env: ParamEnv<'db>,
    term: Term<'db>,
    span: Span,
) -> Option<Vec<Obligation<'db>>> {
    debug_assert_eq!(term, infcx.deeply_resolve_ignoring_regions(term));

    // However, if `term` IS an unresolved inference variable, returns `None`,
    // because we are not able to make any progress at all. This is to prevent
    // cycles where we say "?0 is WF if ?0 is WF".
    if term.is_infer() {
        return None;
    }

    let mut wf = WfPredicates {
        infcx,
        param_env,
        span,
        out: Vec::new(),
        recursion_depth: 0,
        visited_tys: Default::default(),
    };
    wf.add_wf_preds_for_term(term);
    Some(wf.out)
}

impl<'a, 'db> WfPredicates<'a, 'db> {
    fn tcx(&self) -> DbInterner<'db> {
        self.infcx.interner
    }
    fn require_sized(&mut self, subty: Ty<'db>, cause: ()) {
        if !subty.has_escaping_bound_vars() {
            let cause = self.cause(cause);
            let trait_ref = ty::TraitRef::new(
                self.tcx(),
                self.tcx()
                    .require_trait_lang_item(SolverTraitLangItem::Sized),
                [subty],
            );
            self.out.push(Obligation::with_depth(
                self.tcx(),
                cause,
                self.recursion_depth,
                self.param_env,
                ty::Binder::dummy(trait_ref),
            ));
        }
    }
    fn add_wf_preds_for_term(&mut self, term: Term<'db>) {
        term.visit_with(self);
        debug!(?self.out);
    }
    fn nominal_obligations(
        &mut self,
        def_id: DefId,
        args: Args<'db>,
        mut push_obligation: impl FnMut(&mut Self, Obligation<'db>),
    ) {
        // PERF: `Sized`'s predicates include `MetaSized`, but both are compiler implemented marker
        // traits, so `MetaSized` will always be WF if `Sized` is WF and vice-versa. Determining
        // the nominal obligations of `Sized` would in-effect just elaborate `MetaSized` and make
        // the compiler do a bunch of work needlessly.
        if is_wf_lang_item(self.tcx(), def_id, WfLangItem::Sized) {
            return;
        }
        if is_wf_lang_item(self.tcx(), def_id, WfLangItem::ConstParamTy)
            && self.tcx().features().const_param_ty_unchecked()
        {
            return;
        }

        let tcx = self.tcx();
        for clause in tcx.clauses_of(def_id).iter_instantiated(tcx, args) {
            if !clause.has_escaping_bound_vars() {
                let obligation = Obligation::with_depth(
                    tcx,
                    self.cause(()),
                    self.recursion_depth,
                    self.param_env,
                    clause.skip_norm_wip(),
                );
                push_obligation(self, obligation);
            }
        }
    }
    fn add_wf_preds_for_dyn_ty(
        &mut self,
        ty: Ty<'db>,
        data: model::Existentials<'db, super::PreparedProvider<'db>>,
        region: super::Region<'db>,
    ) {
        // Imagine a type like this:
        //
        //     trait Foo { }
        //     trait Bar<'c> : 'c { }
        //
        //     &'b (Foo+'c+Bar<'d>)
        //         ^
        //
        // In this case, the following relationships must hold:
        //
        //     'b <= 'c
        //     'd <= 'c
        //
        // The first conditions is due to the normal region pointer
        // rules, which say that a reference cannot outlive its
        // referent.
        //
        // The final condition may be a bit surprising. In particular,
        // you may expect that it would have been `'c <= 'd`, since
        // usually lifetimes of outer things are conservative
        // approximations for inner things. However, it works somewhat
        // differently with trait objects: here the idea is that if the
        // user specifies a region bound (`'c`, in this case) it is the
        // "master bound" that *implies* that bounds from other traits are
        // all met. (Remember that *all bounds* in a type like
        // `Foo+Bar+Zed` must be met, not just one, hence if we write
        // `Foo<'x>+Bar<'y>`, we know that the type outlives *both* 'x and
        // 'y.)
        //
        // Note: in fact we only permit builtin traits, not `Bar<'d>`, I
        // am looking forward to the future here.
        if !data.has_escaping_bound_vars() && !region.has_escaping_bound_vars() {
            let implicit_bounds = object_region_bounds(self.tcx(), data);

            let explicit_bound = region;

            self.out.reserve(implicit_bounds.len());
            for implicit_bound in implicit_bounds {
                let cause = self.cause(());
                let outlives =
                    ty::Binder::dummy(ty::OutlivesClause(explicit_bound, implicit_bound));
                self.out.push(Obligation::with_depth(
                    self.tcx(),
                    cause,
                    self.recursion_depth,
                    self.param_env,
                    outlives,
                ));
            }

            // We don't add any wf predicates corresponding to the trait ref's generic arguments
            // which allows code like this to compile:
            // ```rust
            // trait Trait<T: Sized> {}
            // fn foo(_: &dyn Trait<[u32]>) {}
            // ```
        }
    }
    fn add_wf_preds_for_pat_ty(
        &mut self,
        base_ty: Ty<'db>,
        pat: model::Pattern<'db, super::PreparedProvider<'db>>,
    ) {
        let tcx = self.tcx();
        match pat.kind() {
            ty::PatternKind::Range { start, end } => {
                let mut check = |c| {
                    let cause = self.cause(());
                    self.out.push(Obligation::with_depth(
                        tcx,
                        cause.clone(),
                        self.recursion_depth,
                        self.param_env,
                        ty::Binder::dummy(ty::PredicateKind::Clause(
                            ty::ClauseKind::ConstArgHasType(c, base_ty),
                        )),
                    ));
                };
                check(start);
                check(end);
            }
            ty::PatternKind::NotNull => {}
            ty::PatternKind::Or(patterns) => {
                for pat in patterns {
                    self.add_wf_preds_for_pat_ty(base_ty, pat)
                }
            }
        }
    }
    fn add_wf_preds_for_inherent_projection(&mut self, data: ty::AliasTerm<DbInterner<'db>>) {
        // An inherent projection is well-formed if
        //
        // (a) its predicates hold (*)
        // (b) its args are wf
        //
        // (*) The predicates of an inherent associated type include the
        //     predicates of the impl that it's contained in.

        // In an ideal world, there are no escaping bound vars here. However, WF is jank, and
        // sometimes there are. We can only `compute_inherent_assoc_term_args` if the Self ty in the
        // args has no escaping bound vars. If we already have impl format args, though,
        // `compute_inherent_assoc_term_args` is a no-op (and we have no Self type), so no need to
        // check for escaping bound vars.
        let can_compute_impl_args =
            matches!(data.kind, ty::AliasTermKind::InherentConstImpl { .. })
                || !data.self_ty().has_escaping_bound_vars();

        if can_compute_impl_args {
            // FIXME(inherent_associated_types): Should this happen inside of a snapshot?
            // FIXME(inherent_associated_types): This is incompatible with the new solver and lazy norm!
            let args = compute_inherent_assoc_term_args(
                &mut SelectionContext::new(self.infcx),
                self.param_env,
                data,
                self.cause(()),
                self.recursion_depth,
                &mut self.out,
            );
            let def_id = data.expect_inherent_def_id();
            self.nominal_obligations(def_id, args, |this, obligation| this.out.push(obligation));
        }

        data.args.visit_with(self);
    }
}
impl<'a, 'db> TypeVisitor<DbInterner<'db>> for WfPredicates<'a, 'db> {
    type Result = ();
    fn visit_ty(&mut self, t: Ty<'db>) -> Self::Result {
        debug!("wf bounds for t={:?} t.kind={:#?}", t, t.kind());

        if !self.visited_tys.insert(t) {
            return;
        }

        let tcx = self.tcx();

        match t.kind() {
            ty::TyKind::Bool
            | ty::TyKind::Char
            | ty::TyKind::Int(..)
            | ty::TyKind::Uint(..)
            | ty::TyKind::Float(..)
            | ty::TyKind::Error(_)
            | ty::TyKind::Str
            | ty::TyKind::CoroutineWitness(..)
            | ty::TyKind::Never
            | ty::TyKind::Param(_)
            | ty::TyKind::Bound(..)
            | ty::TyKind::Placeholder(..)
            | ty::TyKind::Foreign(..) => {
                // WfScalar, WfParameter, etc
            }

            // Can only infer to `ty::TyKind::Int(_) | ty::TyKind::Uint(_)`.
            ty::TyKind::Infer(ty::InferTy::IntVar(_)) => {}

            // Can only infer to `ty::TyKind::Float(_)`.
            ty::TyKind::Infer(ty::InferTy::FloatVar(_)) => {}

            ty::TyKind::Slice(subty) => {
                self.require_sized(subty, ());
            }

            ty::TyKind::Array(subty, len) => {
                self.require_sized(subty, ());
                // Note that the len being WF is implicitly checked while visiting.
                // Here we just check that it's of type usize.
                let cause = self.cause(());
                self.out.push(Obligation::with_depth(
                    tcx,
                    cause,
                    self.recursion_depth,
                    self.param_env,
                    ty::Binder::dummy(ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(
                        len,
                        Ty::new_usize(tcx),
                    ))),
                ));
            }

            ty::TyKind::Pat(base_ty, pat) => {
                self.require_sized(base_ty, ());
                self.add_wf_preds_for_pat_ty(base_ty, pat);
            }

            ty::TyKind::Tuple(tys) => {
                if let Some((last, rest)) = tys.split_last() {
                    for &elem in rest {
                        self.require_sized(elem, ());
                    }
                }
            }

            ty::TyKind::RawPtr(_, _) => {
                // Simple cases that are WF if their type args are WF.
            }

            ty::TyKind::Alias(
                _,
                ty::AliasTy {
                    kind:
                        ty::AliasTyKind::Projection { def_id }
                        | ty::AliasTyKind::Opaque { def_id }
                        | ty::AliasTyKind::Free { def_id },
                    args,
                    ..
                },
            ) => {
                self.nominal_obligations(def_id, args, |this, obligation| {
                    this.out.push(obligation)
                });
            }
            ty::TyKind::Alias(
                _,
                data @ ty::AliasTy {
                    kind: ty::AliasTyKind::Inherent { .. },
                    ..
                },
            ) => {
                self.add_wf_preds_for_inherent_projection(data.into());
                return; // Subtree handled by compute_inherent_projection.
            }

            ty::TyKind::Adt(def, args) => {
                // WfNominalType
                self.nominal_obligations(def.def_id(), args, |this, obligation| {
                    this.out.push(obligation)
                });
            }

            ty::TyKind::FnDef(did, args) => {
                let args = args.no_bound_vars().unwrap();
                // HACK: Check the return type of function definitions for
                // well-formedness to mostly fix #84533. This is still not
                // perfect and there may be ways to abuse the fact that we
                // ignore requirements with escaping bound vars. That's a
                // more general issue however.
                let fn_sig = tcx.fn_sig(did).instantiate(tcx, args).skip_norm_wip();
                fn_sig.output().skip_binder().visit_with(self);

                self.nominal_obligations(did, args, |this, obligation| this.out.push(obligation));
            }

            ty::TyKind::Ref(r, rty, _) => {
                // WfReference
                if !r.has_escaping_bound_vars() && !rty.has_escaping_bound_vars() {
                    let cause = self.cause(());
                    self.out.push(Obligation::with_depth(
                        tcx,
                        cause,
                        self.recursion_depth,
                        self.param_env,
                        ty::Binder::dummy(ty::PredicateKind::Clause(ty::ClauseKind::TypeOutlives(
                            ty::OutlivesClause(rty, r),
                        ))),
                    ));
                }
            }

            ty::TyKind::Coroutine(did, args, ..) => {
                // Walk ALL the types in the coroutine: this will
                // include the upvar types as well as the yield
                // type. Note that this is mildly distinct from
                // the closure case, where we have to be careful
                // about the signature of the closure. We don't
                // have the problem of implied bounds here since
                // coroutines don't take arguments.
                self.nominal_obligations(did, args, |this, obligation| this.out.push(obligation));
            }

            ty::TyKind::Closure(did, args) => {
                // Note that we cannot skip the generic types
                // types. Normally, within the fn
                // body where they are created, the generics will
                // always be WF, and outside of that fn body we
                // are not directly inspecting closure types
                // anyway, except via auto trait matching (which
                // only inspects the upvar types).
                // But when a closure is part of a type-alias-impl-trait
                // then the function that created the defining site may
                // have had more bounds available than the type alias
                // specifies. This may cause us to have a closure in the
                // hidden type that is not actually well formed and
                // can cause compiler crashes when the user abuses unsafe
                // code to procure such a closure.
                // See tests/ui/type-alias-impl-trait/wf_check_closures.rs
                self.nominal_obligations(did, args, |this, obligation| this.out.push(obligation));
                // Only check the upvar types for WF, not the rest
                // of the types within. This is needed because we
                // capture the signature and it may not be WF
                // without the implied bounds. Consider a closure
                // like `|x: &'a T|` -- it may be that `T: 'a` is
                // not known to hold in the creator's context (and
                // indeed the closure may not be invoked by its
                // creator, but rather turned to someone who *can*
                // verify that).
                //
                // The special treatment of closures here really
                // ought not to be necessary either; the problem
                // is related to #25860 -- there is no way for us
                // to express a fn type complete with the implied
                // bounds that it is assuming. I think in reality
                // the WF rules around fn are a bit messed up, and
                // that is the rot problem: `fn(&'a T)` should
                // probably always be WF, because it should be
                // shorthand for something like `where(T: 'a) {
                // fn(&'a T) }`, as discussed in #25860.
                let upvars = args.as_closure().tupled_upvars_ty();
                return upvars.visit_with(self);
            }

            ty::TyKind::CoroutineClosure(did, args) => {
                // See the above comments. The same apply to coroutine-closures.
                self.nominal_obligations(did, args, |this, obligation| this.out.push(obligation));
                let upvars = args.as_coroutine_closure().tupled_upvars_ty();
                return upvars.visit_with(self);
            }

            ty::TyKind::FnPtr(..) => {
                // Let the visitor iterate into the argument/return
                // types appearing in the fn signature.
            }
            ty::TyKind::UnsafeBinder(ty) => {
                // FIXME(unsafe_binders): For now, we have no way to express
                // that a type must be `ManuallyDrop` OR `Copy` (or a pointer).
                if !ty.has_escaping_bound_vars() {
                    self.out.push(Obligation::new(
                        self.tcx(),
                        self.cause(()),
                        self.param_env,
                        ty.map_bound(|ty| {
                            ty::TraitRef::new(
                                self.tcx(),
                                self.tcx().require_trait_lang_item(
                                    SolverTraitLangItem::BikeshedGuaranteedNoDrop,
                                ),
                                [ty],
                            )
                        }),
                    ));
                }

                // We recurse into the binder below.
            }

            ty::TyKind::Dynamic(data, r) => {
                // WfObject
                //
                // Here, we defer WF checking due to higher-ranked
                // regions. This is perhaps not ideal.
                self.add_wf_preds_for_dyn_ty(t, data, r);

                // FIXME(#27579) RFC also considers adding trait
                // obligations that don't refer to Self and
                // checking those
                if let Some(principal) = data.principal() {
                    let principal_def_id = principal.skip_binder().def_id;
                    self.out.push(Obligation::with_depth(
                        tcx,
                        self.cause(()),
                        self.recursion_depth,
                        self.param_env,
                        ty::Binder::dummy(ty::PredicateKind::DynCompatible(principal_def_id)),
                    ));

                    // For the most part we don't add wf predicates corresponding to
                    // the trait ref's generic arguments which allows code like this
                    // to compile:
                    // ```rust
                    // trait Trait<T: Sized> {}
                    // fn foo(_: &dyn Trait<[u32]>) {}
                    // ```
                    //
                    // However, we sometimes incidentally check that const arguments
                    // have the correct type as a side effect of the anon const
                    // desugaring. To make this "consistent" for users we explicitly
                    // check `ConstArgHasType` clauses so that const args that don't
                    // go through an anon const still have their types checked.
                    //
                    // See also: https://rustc-dev-guide.rust-lang.org/const-generics.html
                    let args = principal.skip_binder().with_self_ty(self.tcx(), t).args;
                    self.nominal_obligations(principal_def_id, args, |this, obligation| {
                        let kind = obligation.predicate.kind().skip_binder();
                        let keep = match kind {
                            ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(ct, _))
                                if matches!(
                                    ct.kind(),
                                    ty::ConstKind::Param(..) | ty::ConstKind::Placeholder(..)
                                ) =>
                            {
                                // ConstArgHasType clauses are not higher kinded. Assert as
                                // such so we can fix this up if that ever changes.
                                assert!(obligation.predicate.kind().bound_vars().is_empty());
                                // In stable rust, variables from the trait object binder
                                // cannot be referenced by a ConstArgHasType clause. However,
                                // under `generic_const_parameter_types`, it can. Ignore those
                                // predicates for now, to not have HKT-ConstArgHasTypes.
                                !kind.has_escaping_bound_vars()
                            }
                            _ => false,
                        };
                        if keep {
                            this.out.push(obligation);
                        }
                    });
                }

                if !t.has_escaping_bound_vars() {
                    for projection in data.projection_bounds() {
                        let pred_binder = projection
                            .with_self_ty(tcx, t)
                            .map_bound(|p| {
                                p.term.as_const().map(|ct| {
                                    let assoc_const_ty = tcx
                                        .type_of(p.def_id())
                                        .instantiate(tcx, p.projection_term.args)
                                        .skip_norm_wip();
                                    ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(
                                        ct,
                                        assoc_const_ty,
                                    ))
                                })
                            })
                            .transpose();
                        if let Some(pred_binder) = pred_binder {
                            self.out.push(Obligation::with_depth(
                                tcx,
                                self.cause(()),
                                self.recursion_depth,
                                self.param_env,
                                pred_binder,
                            ));
                        }
                    }
                }
            }

            // Inference variables are the complicated case, since we don't
            // know what type they are. We do two things:
            //
            // 1. Check if they have been resolved, and if so proceed with
            //    THAT type.
            // 2. If not, we've at least simplified things (e.g., we went
            //    from `Vec?0>: WF` to `?0: WF`), so we can
            //    register a pending obligation and keep
            //    moving. (Goal is that an "inductive hypothesis"
            //    is satisfied to ensure termination.)
            // See also the comment on `fn obligations`, describing cycle
            // prevention, which happens before this can be reached.
            ty::TyKind::Infer(_) => {
                let cause = self.cause(());
                self.out.push(Obligation::with_depth(
                    tcx,
                    cause,
                    self.recursion_depth,
                    self.param_env,
                    ty::Binder::dummy(ty::PredicateKind::Clause(ty::ClauseKind::WellFormed(
                        t.into(),
                    ))),
                ));
            }
        }

        t.super_visit_with(self)
    }

    fn visit_const(&mut self, c: Const<'db>) -> Self::Result {
        let tcx = self.tcx();

        match c.kind() {
            ty::ConstKind::Alias(_, alias_const) => {
                if !c.has_escaping_bound_vars() {
                    // Skip type consts as mGCA doesn't support evaluatable clauses
                    if !alias_const.kind.is_direct_const(tcx)
                        && !ty::inherent::Features::<DbInterner<'db>>::gca_const_items(
                            tcx.features(),
                        )
                    {
                        let predicate = ty::Binder::dummy(ty::PredicateKind::Clause(
                            ty::ClauseKind::ConstEvaluatable(c),
                        ));
                        let cause = self.cause(());
                        self.out.push(Obligation::with_depth(
                            tcx,
                            cause,
                            self.recursion_depth,
                            self.param_env,
                            predicate,
                        ));
                    }

                    match alias_const.kind {
                        ty::AliasConstKind::InherentSelf { .. } => {
                            self.add_wf_preds_for_inherent_projection(alias_const.into());
                            return; // Subtree is handled by above function
                        }
                        // FIXME: This should be unreachable but isn't because we normalize in item
                        // wfck before computing wf requirements
                        ty::AliasConstKind::InherentImpl { .. } => {
                            self.add_wf_preds_for_inherent_projection(alias_const.into());
                            return;
                        }
                        ty::AliasConstKind::Projection { def_id }
                        | ty::AliasConstKind::Free { def_id }
                        | ty::AliasConstKind::Anon { def_id } => {
                            self.nominal_obligations(
                                def_id,
                                alias_const.args,
                                |this, obligation| this.out.push(obligation),
                            );
                        }
                    }
                }
            }
            ty::ConstKind::Infer(_) => {
                let cause = self.cause(());

                self.out.push(Obligation::with_depth(
                    tcx,
                    cause,
                    self.recursion_depth,
                    self.param_env,
                    ty::Binder::dummy(ty::PredicateKind::Clause(ty::ClauseKind::WellFormed(
                        c.into(),
                    ))),
                ));
            }
            ty::ConstKind::Expr(_) => {
                // FIXME(generic_const_exprs): this doesn't verify that given `Expr(N + 1)` the
                // trait bound `typeof(N): Add<typeof(1)>` holds. This is currently unnecessary
                // as `ConstKind::Expr` is only produced via normalization of `ConstKind::Alias`
                // which means that the `DefId` would have been typeck'd elsewhere. However in
                // the future we may allow directly lowering to `ConstKind::Expr` in which case
                // we would not be proving bounds we should.

                let predicate = ty::Binder::dummy(ty::PredicateKind::Clause(
                    ty::ClauseKind::ConstEvaluatable(c),
                ));
                let cause = self.cause(());
                self.out.push(Obligation::with_depth(
                    tcx,
                    cause,
                    self.recursion_depth,
                    self.param_env,
                    predicate,
                ));
            }

            ty::ConstKind::Error(_)
            | ty::ConstKind::Param(_)
            | ty::ConstKind::Bound(..)
            | ty::ConstKind::Placeholder(..) => {
                // These variants are trivially WF, so nothing to do here.
            }
            ty::ConstKind::Value(val) => {
                // FIXME(mgca): no need to feature-gate once valtree lifetimes are not erased
                if tcx.features().gca() {
                    match val.ty.kind() {
                        ty::TyKind::Adt(adt_def, args) => {
                            let adt_val = val.destructure_adt_const();
                            let variant_def = adt_def.variant(adt_val.variant);
                            let cause = self.cause(());
                            self.out
                                .extend(variant_def.fields.iter().zip(adt_val.fields).map(
                                    |(field_def, &field_val)| {
                                        let field_ty = field_def.ty(tcx, args).skip_norm_wip();
                                        let predicate = ty::PredicateKind::Clause(
                                            ty::ClauseKind::ConstArgHasType(field_val, field_ty),
                                        );
                                        Obligation::with_depth(
                                            tcx,
                                            cause.clone(),
                                            self.recursion_depth,
                                            self.param_env,
                                            predicate,
                                        )
                                    },
                                ));
                        }
                        ty::TyKind::Tuple(field_tys) => {
                            let field_vals = val.to_branch();
                            let cause = self.cause(());
                            self.out.extend(field_tys.iter().zip(field_vals).map(
                                |(field_ty, &field_val)| {
                                    let predicate = ty::PredicateKind::Clause(
                                        ty::ClauseKind::ConstArgHasType(field_val, field_ty),
                                    );
                                    Obligation::with_depth(
                                        tcx,
                                        cause.clone(),
                                        self.recursion_depth,
                                        self.param_env,
                                        predicate,
                                    )
                                },
                            ));
                        }
                        ty::TyKind::Array(elem_ty, _len) => {
                            let elem_vals = val.to_branch();
                            let cause = self.cause(());

                            self.out.extend(elem_vals.iter().map(|&elem_val| {
                                let predicate = ty::PredicateKind::Clause(
                                    ty::ClauseKind::ConstArgHasType(elem_val, elem_ty),
                                );
                                Obligation::with_depth(
                                    tcx,
                                    cause.clone(),
                                    self.recursion_depth,
                                    self.param_env,
                                    predicate,
                                )
                            }));
                        }
                        _ => {}
                    }
                }

                // FIXME: Enforce that values are structurally-matchable.
            }
        }

        c.super_visit_with(self)
    }

    fn visit_predicate<P: PredicateProxy<DbInterner<'db>>>(&mut self, _p: P) -> Self::Result {
        panic!("predicate should not be checked for well-formedness");
    }
}

/// Given an object type like `SomeTrait + Send`, computes the lifetime
/// bounds that must hold on the elided self type. These are derived
/// from the declarations of `SomeTrait`, `Send`, and friends -- if
/// they declare `trait SomeTrait : 'static`, for example, then
/// `'static` would appear in the list.
///
/// N.B., in some cases, particularly around higher-ranked bounds,
/// this function returns a kind of conservative approximation.
/// That is, all regions returned by this function are definitely
/// required, but there may be other region bounds that are not
/// returned, as well as requirements like `for<'a> T: 'a`.
///
/// Requires that trait definitions have been processed so that we can
/// elaborate predicates and walk supertraits.
pub fn object_region_bounds<'db>(
    tcx: DbInterner<'db>,
    existential_predicates: model::Existentials<'db, super::PreparedProvider<'db>>,
) -> Vec<super::Region<'db>> {
    let erased_self_ty = tcx.ty(ty::TyKind::Infer(ty::InferTy::FreshTy(0)));

    let clauses = existential_predicates
        .iter()
        .map(|predicate| predicate.with_self_ty(tcx, erased_self_ty));

    ty::elaborate::elaborate(tcx, clauses)
        .filter_map(|clause| {
            debug!(?clause);
            match clause.kind().skip_binder() {
                ty::ClauseKind::TypeOutlives(ty::OutlivesClause(ref t, ref r)) => {
                    // Search for a bound of the form `erased_self_ty
                    // : 'a`, but be wary of something like `for<'a>
                    // erased_self_ty : 'a` (we interpret a
                    // higher-ranked bound like that as 'static,
                    // though at present the code in `fulfill.rs`
                    // considers such bounds to be unsatisfiable, so
                    // it's kind of a moot point since you could never
                    // construct such an object, but this seems
                    // correct even if that code changes).
                    if t == &erased_self_ty && !r.has_escaping_bound_vars() {
                        Some(*r)
                    } else {
                        None
                    }
                }
                ty::ClauseKind::Trait(_)
                | ty::ClauseKind::HostEffect(..)
                | ty::ClauseKind::RegionOutlives(_)
                | ty::ClauseKind::Projection(_)
                | ty::ClauseKind::ConstArgHasType(_, _)
                | ty::ClauseKind::WellFormed(_)
                | ty::ClauseKind::UnstableFeature(_)
                | ty::ClauseKind::ConstEvaluatable(_) => None,
            }
        })
        .collect()
}
fn compute_inherent_assoc_term_args<'a, 'b, 'db>(
    selcx: &'a mut SelectionContext<'b, 'db>,
    param_env: ParamEnv<'db>,
    alias_term: ty::AliasTerm<DbInterner<'db>>,
    cause: (),
    depth: usize,
    obligations: &mut Vec<Obligation<'db>>,
) -> Args<'db> {
    let tcx = selcx.tcx();

    let alias_def_id = match alias_term.kind {
        ty::AliasTermKind::InherentTy { def_id } => def_id,
        ty::AliasTermKind::InherentConstSelf { def_id } => def_id,
        ty::AliasTermKind::InherentConstImpl { .. } => return alias_term.args,
        kind => panic!("expected inherent alias, found {kind:?}"),
    };

    let impl_def_id = tcx.inherent_alias_term_parent(alias_def_id);
    let impl_args = ty::InferCtxtLike::fresh_args_for_item(selcx.infcx, impl_def_id);

    let impl_ty = tcx.type_of(impl_def_id).instantiate(tcx, impl_args);
    let impl_ty = { impl_ty.skip_norm_wip() };

    // Infer the generic parameters of the impl by unifying the
    // impl type with the self type of the projection.
    let self_ty = ty::Unnormalized::<DbInterner<'db>, _>::new_wip(alias_term.self_ty());
    let self_ty = { self_ty.skip_normalization() };

    match rustc_type_ir::relate::solver_relating::RelateExt::relate(
        selcx.infcx,
        param_env,
        impl_ty,
        ty::Invariant,
        self_ty,
        <Span as ty::inherent::Span<DbInterner<'db>>>::dummy(),
    ) {
        Ok(goals) => obligations.extend(goals.into_iter().map(Obligation::from_goal)),
        Err(_) => {
            panic!("inherent Self types differ after selection: {self_ty:?}, {impl_ty:?}");
        }
    }

    alias_term.rebase_inherent_args_onto_impl(impl_args, tcx)
}

impl<'db> Term<'db> {
    fn is_infer(&self) -> bool {
        match self.kind() {
            TermKind::Ty(ty) => ty.is_ty_var(),
            TermKind::Const(ct) => ct.is_ct_infer(),
        }
    }
    fn as_const(&self) -> Option<Const<'db>> {
        if let TermKind::Const(c) = self.kind() {
            Some(c)
        } else {
            None
        }
    }
}

impl model::FeatureSet<'_> {
    fn gca(&self) -> bool {
        self.gca_min_const_items() || self.gca_adts()
    }
    fn gca_min_const_items(&self) -> bool {
        self.enabled(model::Symbol("gca_min_const_items"))
    }
    fn gca_adts(&self) -> bool {
        self.enabled(model::Symbol("gca_adts"))
    }
    fn const_param_ty_unchecked(&self) -> bool {
        self.enabled(model::Symbol("const_param_ty_unchecked"))
    }
    fn enabled(&self, feature: model::Symbol<'_>) -> bool {
        if self.enabled.contains(&feature) {
            true
        } else {
            false
        }
    }
}

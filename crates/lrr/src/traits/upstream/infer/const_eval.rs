//! Pinned next-solver constant normalization; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_const_eval.py.
//! Compiler CTFE query leaves are typed immutable preparation inputs, not a
//! callback evaluator. Missing leaves are explicit admission dependencies.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Span, Ty};
use crate::traits::upstream::{
    facts::SourceFacts as _,
    identity::DefinitionId as DefId,
    model::{self, ErrorToken},
};
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::{
    self as ty, Interner, MayBeErased, TypeFoldable, TypeFolder, TypeSuperFoldable,
    TypeVisitableExt, TypingMode, inherent::*,
};
use std::{collections::HashMap, fmt::Debug};
use tracing::debug;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type Args<'db> = model::Args<'db, PreparedProvider<'db>>;
type ValTree<'db> = model::ValTree<'db, PreparedProvider<'db>>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TypingEnv<'db> {
    typing_mode: ty::TypingModeEqWrapper<DbInterner<'db>>,
    pub param_env: ParamEnv<'db>,
}

/// Exact client query identity, including its definition diagnostic span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ConstEvalQuery<'db> {
    pub(crate) typing_env: TypingEnv<'db>,
    pub(crate) definition: DefId,
    pub(crate) constant: ty::AliasConst<DbInterner<'db>>,
    pub(crate) span: Span,
}
/// Compiler ErrorHandled guarantee/span representation at the typed boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstEvalError {
    Reported(ErrorToken, Span),
    TooGeneric(Span),
}
pub(crate) type ConstEvalResult<'db> = Result<Result<ValTree<'db>, Ty<'db>>, ConstEvalError>;

/// Required source/query data; never a semantic None or NoSolution answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConstEvalDependency<'db> {
    PostAnalysisEnv(DefId),
    Evaluation(ConstEvalQuery<'db>),
}

/// Immutable terminal inputs only. Construction grants no admission/readiness.
/// The real producer must retain actual source/query provenance/currentness;
/// it remains unimplemented and root entry stays held until that closure exists.
pub(crate) struct PreparedConstEvalInputs<'db> {
    post_analysis_envs: HashMap<DefId, ParamEnv<'db>>,
    evaluations: HashMap<ConstEvalQuery<'db>, ConstEvalResult<'db>>,
}
impl<'db> PreparedConstEvalInputs<'db> {
    pub(crate) fn new(
        post_analysis_envs: HashMap<DefId, ParamEnv<'db>>,
        evaluations: HashMap<ConstEvalQuery<'db>, ConstEvalResult<'db>>,
    ) -> Self {
        Self {
            post_analysis_envs,
            evaluations,
        }
    }
    pub(crate) fn post_analysis_env(
        &self,
        definition: DefId,
    ) -> Result<ParamEnv<'db>, ConstEvalDependency<'db>> {
        self.post_analysis_envs
            .get(&definition)
            .copied()
            .ok_or(ConstEvalDependency::PostAnalysisEnv(definition))
    }
    pub(crate) fn evaluation(
        &self,
        query: ConstEvalQuery<'db>,
    ) -> Result<ConstEvalResult<'db>, ConstEvalDependency<'db>> {
        self.evaluations
            .get(&query)
            .copied()
            .ok_or(ConstEvalDependency::Evaluation(query))
    }
}

#[derive(Debug)]
enum EvaluateConstErr<'db, E> {
    /// The constant being evaluated was either a generic parameter or inference variable, *or*,
    /// some alias const with either generic parameters or inference variables in its
    /// generic arguments.
    HasGenericsOrInfers,
    /// The type this constant evaluated to is not valid for use in const generics. This should
    /// always result in an error when checking the constant is correctly typed for the parameter
    /// it is an argument to, so a bug is delayed when encountering this.
    InvalidConstParamTy(ErrorToken),
    /// CTFE failed to evaluate the constant in some unrecoverable way (e.g. encountered a `panic!`).
    /// This is also used when the constant was already tainted by error.
    EvaluationFailure(ErrorToken),
    FailedNormalization(E),
    /// Required prepared input was not admitted; never a semantic failure.
    Preparation(ConstEvalDependency<'db>),
}
pub(crate) fn evaluate_const<'db, E: Debug>(
    infcx: &InferenceContext<'db>,
    param_env: ParamEnv<'db>,
    alias_const: ty::AliasConst<DbInterner<'db>>,
    normalize_ty: impl FnOnce(ty::Unnormalized<DbInterner<'db>, Ty<'db>>) -> Result<Ty<'db>, E>,
) -> Result<Result<Option<Const<'db>>, E>, ConstEvalDependency<'db>> {
    let ct = ty::Const::new_alias(infcx.interner, ty::IsRigid::No, alias_const);

    match try_evaluate_const(infcx, ct, param_env, normalize_ty) {
        Ok(ct) => Ok(Ok(Some(ct))),
        Err(EvaluateConstErr::EvaluationFailure(e)) => {
            Ok(Ok(Some(ty::Const::new_error(infcx.interner, e))))
        }
        Err(EvaluateConstErr::InvalidConstParamTy(_) | EvaluateConstErr::HasGenericsOrInfers) => {
            Ok(Ok(None))
        }
        Err(EvaluateConstErr::FailedNormalization(e)) => Ok(Err(e)),
        Err(EvaluateConstErr::Preparation(dependency)) => Err(dependency),
    }
}
fn try_evaluate_const<'db, E: Debug>(
    infcx: &InferenceContext<'db>,
    ct: Const<'db>,
    param_env: ParamEnv<'db>,
    normalize_ty: impl FnOnce(ty::Unnormalized<DbInterner<'db>, Ty<'db>>) -> Result<Ty<'db>, E>,
) -> Result<Const<'db>, EvaluateConstErr<'db, E>> {
    let tcx = infcx.interner;
    let ct = infcx.deeply_resolve_ignoring_regions(ct);
    debug!(?ct);

    match ct.kind() {
        ty::ConstKind::Value(..) => Ok(ct),
        ty::ConstKind::Error(e) => Err(EvaluateConstErr::EvaluationFailure(e)),
        ty::ConstKind::Param(_)
        | ty::ConstKind::Infer(_)
        | ty::ConstKind::Bound(_, _)
        | ty::ConstKind::Placeholder(_)
        | ty::ConstKind::Expr(_) => Err(EvaluateConstErr::HasGenericsOrInfers),
        ty::ConstKind::Alias(_, alias_const) => {
            let opt_anon_const_kind = match alias_const.kind {
                ty::AliasConstKind::Anon { def_id } => Some((def_id, tcx.anon_const_kind(def_id))),
                _ => None,
            };

            // Postpone evaluation of constants that depend on generic parameters or
            // inference variables.
            //
            // We use `TypingMode::PostAnalysis` here which is not *technically* correct
            // to be revealing opaque types here as borrowcheck has not run yet. However,
            // CTFE itself uses `TypingMode::PostAnalysis` unconditionally even during
            // typeck and not doing so has a lot of (undesirable) fallout (#101478, #119821).
            // As a result we always use a revealed env when resolving the instance to evaluate.
            //
            // FIXME: `const_eval_resolve_for_typeck` should probably just modify the env itself
            // instead of having this logic here
            let (args, typing_env) = match opt_anon_const_kind {
                // We handle `generic_const_exprs` separately as reasonable ways of handling constants in the type system
                // completely fall apart under `generic_const_exprs` and makes this whole function Really hard to reason
                // about if you have to consider gce whatsoever.
                Some((def_id, ty::AnonConstKind::GCE)) => {
                    if alias_const.has_non_region_infer() || alias_const.has_non_region_param() {
                        // `feature(generic_const_exprs)` causes anon consts to inherit all parent generics. This can cause
                        // inference variables and generic parameters to show up in `ty::Const` even though the anon const
                        // does not actually make use of them. We handle this case specially and attempt to evaluate anyway.
                        match tcx.services().abstract_const(tcx, def_id) {
                            Ok(Some(ct)) => {
                                let ct = tcx.expand_abstract_consts(
                                    ct.instantiate(tcx, alias_const.args).skip_norm_wip(),
                                );
                                if let Err(e) = ct.error_reported() {
                                    return Err(EvaluateConstErr::EvaluationFailure(e));
                                } else if ct.has_non_region_infer() || ct.has_non_region_param() {
                                    // If the anon const *does* actually use generic parameters or inference variables from
                                    // the generic arguments provided for it, then we should *not* attempt to evaluate it.
                                    return Err(EvaluateConstErr::HasGenericsOrInfers);
                                } else {
                                    let args = replace_param_and_infer_args_with_placeholder(
                                        tcx,
                                        alias_const.args,
                                    );
                                    let typing_env = infcx
                                        .typing_env(tcx.erase_and_anonymize_regions(param_env))
                                        .with_post_analysis_normalized(tcx);
                                    (args, typing_env)
                                }
                            }
                            Err(_) | Ok(None) => {
                                let args = Args::identity_for_item(tcx, def_id);
                                let typing_env = TypingEnv::post_analysis(tcx, def_id)
                                    .map_err(EvaluateConstErr::Preparation)?;
                                (args, typing_env)
                            }
                        }
                    } else {
                        let typing_env = infcx
                            .typing_env(tcx.erase_and_anonymize_regions(param_env))
                            .with_post_analysis_normalized(tcx);
                        (alias_const.args, typing_env)
                    }
                }
                Some((def_id, ty::AnonConstKind::RepeatExprCount)) => {
                    if alias_const.has_non_region_infer() {
                        // Diagnostics will sometimes replace the identity args of anon consts in
                        // array repeat expr counts with inference variables so we have to handle this
                        // even though it is not something we should ever actually encounter.
                        //
                        // Array repeat expr counts are allowed to syntactically use generic parameters
                        // but must not actually depend on them in order to evalaute successfully. This means
                        // that it is actually fine to evalaute them in their own environment rather than with
                        // the actually provided generic arguments.
                        tcx.delay_bug("AnonConst with infer args but no error reported");
                    }

                    // The generic args of repeat expr counts under `min_const_generics` are not supposed to
                    // affect evaluation of the constant as this would make it a "truly" generic const arg.
                    // To prevent this we discard all the generic arguments and evalaute with identity args
                    // and in its own environment instead of the current environment we are normalizing in.
                    let args = Args::identity_for_item(tcx, def_id);
                    let typing_env = TypingEnv::post_analysis(tcx, def_id)
                        .map_err(EvaluateConstErr::Preparation)?;

                    (args, typing_env)
                }
                Some((
                    _,
                    ty::AnonConstKind::MCG
                    | ty::AnonConstKind::NonTypeSystemAnon
                    | ty::AnonConstKind::NonTypeSystemInline,
                ))
                | None => {
                    // We are only dealing with "truly" generic/uninferred constants here:
                    // - GCEConsts have been handled separately
                    // - Repeat expr count back compat consts have also been handled separately
                    // So we are free to simply defer evaluation here.
                    //
                    // FIXME: This assumes that `args` are normalized which is not necessarily true
                    //
                    // Const patterns are converted to type system constants before being
                    // evaluated. However, we don't care about them here as pattern evaluation
                    // logic does not go through type system normalization. If it did this would
                    // be a backwards compatibility problem as we do not enforce "syntactic" non-
                    // usage of generic parameters like we do here.
                    if alias_const.args.has_non_region_param()
                        || alias_const.args.has_non_region_infer()
                        || alias_const.args.has_non_region_placeholders()
                    {
                        return Err(EvaluateConstErr::HasGenericsOrInfers);
                    }

                    // Since there is no generic parameter, we can just drop the environment
                    // to prevent query cycle.
                    let typing_env = TypingEnv::fully_monomorphized(tcx);

                    (alias_const.args, typing_env)
                }
            };

            let alias_const = ty::AliasConst::new(tcx, alias_const.kind, args);
            let erased_alias_const = tcx.erase_and_anonymize_regions(alias_const);

            // FIXME: `def_span` will point at the definition of this const; ideally, we'd point at
            // where it gets used as a const generic.
            let span = alias_const.kind.def_span(tcx);
            match tcx
                .const_eval_resolve_for_typeck(typing_env, erased_alias_const, span)
                .map_err(EvaluateConstErr::Preparation)?
            {
                Ok(Ok(val)) => {
                    let ty = normalize_ty(alias_const.type_of(tcx))
                        .map_err(EvaluateConstErr::FailedNormalization)?;
                    Ok(ty::Const::new(
                        tcx,
                        ty::ConstKind::Value(model::ValueConst { ty, tree: val }),
                    ))
                }
                Ok(Err(_)) => {
                    let e = tcx.delay_bug(
                        "Type system constant with non valtree'able type evaluated but no error emitted",
                    );
                    Err(EvaluateConstErr::InvalidConstParamTy(e))
                }
                Err(ConstEvalError::Reported(info, _)) => {
                    Err(EvaluateConstErr::EvaluationFailure(info.into()))
                }
                Err(ConstEvalError::TooGeneric(_)) => Err(EvaluateConstErr::HasGenericsOrInfers),
            }
        }
    }
}
fn replace_param_and_infer_args_with_placeholder<'db>(
    tcx: DbInterner<'db>,
    args: Args<'db>,
) -> Args<'db> {
    struct ReplaceParamAndInferWithPlaceholder<'db> {
        tcx: DbInterner<'db>,
        idx: ty::BoundVar,
    }

    impl<'db> TypeFolder<DbInterner<'db>> for ReplaceParamAndInferWithPlaceholder<'db> {
        fn cx(&self) -> DbInterner<'db> {
            self.tcx
        }

        fn fold_ty(&mut self, t: Ty<'db>) -> Ty<'db> {
            if let ty::TyKind::Infer(_) = t.kind() {
                let idx = self.idx;
                self.idx += 1;
                Ty::new_placeholder(
                    self.tcx,
                    ty::PlaceholderType::new(
                        ty::UniverseIndex::ROOT,
                        ty::BoundTy {
                            var: idx,
                            kind: ty::BoundTyKind::Anon,
                        },
                    ),
                )
            } else {
                t.super_fold_with(self)
            }
        }

        fn fold_const(&mut self, c: Const<'db>) -> Const<'db> {
            if let ty::ConstKind::Infer(_) = c.kind() {
                let idx = self.idx;
                self.idx += 1;
                ty::Const::new_placeholder(
                    self.tcx,
                    ty::PlaceholderConst::new(ty::UniverseIndex::ROOT, ty::BoundConst::new(idx)),
                )
            } else {
                c.super_fold_with(self)
            }
        }
    }

    args.fold_with(&mut ReplaceParamAndInferWithPlaceholder {
        tcx,
        idx: ty::BoundVar::ZERO,
    })
}
impl<'db> InferenceContext<'db> {
    pub fn typing_env(&self, param_env: ParamEnv<'db>) -> TypingEnv<'db> {
        let typing_mode = match self.typing_mode_raw() {
            // FIXME(#132279): This erases the `defining_opaque_types` as it isn't possible
            // to handle them without proper canonicalization. This means we may cause cycle
            // errors and fail to reveal opaques while inside of bodies. We should rename this
            // function and require explicit comments on all use-sites in the future.
            ty::TypingMode::Typeck {
                defining_opaque_types_and_generators: _,
            }
            | ty::TypingMode::PostTypeckUntilBorrowck {
                defining_opaque_types: _,
            } => TypingMode::non_body_analysis(),
            mode @ (ty::TypingMode::Coherence
            | ty::TypingMode::PostBorrowck { .. }
            | ty::TypingMode::PostAnalysis
            | ty::TypingMode::Reflection
            | ty::TypingMode::Codegen) => mode,
            ty::TypingMode::ErasedNotCoherence(MayBeErased) => unreachable!(),
        };
        TypingEnv::new(param_env, typing_mode)
    }
}
impl<'db> TypingEnv<'db> {
    pub fn new(param_env: ParamEnv<'db>, typing_mode: ty::TypingMode<DbInterner<'db>>) -> Self {
        Self {
            typing_mode: ty::TypingModeEqWrapper(typing_mode),
            param_env,
        }
    }
    pub fn fully_monomorphized(tcx: DbInterner<'db>) -> TypingEnv<'db> {
        Self::new(
            model::ParamEnv(tcx.clauses(model::List::default())),
            TypingMode::Codegen,
        )
    }
    pub fn post_analysis(
        tcx: DbInterner<'db>,
        def_id: DefId,
    ) -> Result<TypingEnv<'db>, ConstEvalDependency<'db>> {
        Ok(TypingEnv::new(
            param_env_normalized_for_post_analysis(tcx, def_id)?,
            TypingMode::PostAnalysis,
        ))
    }
    pub fn with_post_analysis_normalized(self, _tcx: DbInterner<'db>) -> TypingEnv<'db> {
        let TypingEnv {
            typing_mode,
            param_env,
        } = self;
        match typing_mode.0.assert_not_erased() {
            TypingMode::Coherence
            | TypingMode::Reflection
            | TypingMode::Typeck { .. }
            | TypingMode::PostTypeckUntilBorrowck { .. }
            | TypingMode::PostBorrowck { .. } => {}
            TypingMode::PostAnalysis | TypingMode::Codegen => return self,
        }

        let param_env = param_env;
        TypingEnv::new(param_env, TypingMode::PostAnalysis)
    }
}
fn param_env_normalized_for_post_analysis<'db>(
    tcx: DbInterner<'db>,
    def_id: DefId,
) -> Result<ParamEnv<'db>, ConstEvalDependency<'db>> {
    tcx.services().const_eval_inputs().post_analysis_env(def_id)
}
impl<'db> DbInterner<'db> {
    pub fn const_eval_resolve_for_typeck(
        self,
        typing_env: TypingEnv<'db>,
        ct: ty::AliasConst<DbInterner<'db>>,
        span: Span,
    ) -> Result<ConstEvalResult<'db>, ConstEvalDependency<'db>> {
        // Cannot resolve `Unevaluated` constants that contain inference
        // variables. We reject those here since `resolve`
        // would fail otherwise.
        //
        // When trying to evaluate constants containing inference variables,
        // use `Infcx::const_eval_resolve` instead.
        if ct.args.has_non_region_infer() {
            panic!("did not expect inference variables here");
        }

        let def_id = match ct.kind {
            ty::AliasConstKind::InherentSelf { .. } => {
                panic!("got AliasConstKind::InherentSelf in const_eval_resolve_for_typeck")
            }
            ty::AliasConstKind::Projection { def_id }
            | ty::AliasConstKind::InherentImpl { def_id }
            | ty::AliasConstKind::Free { def_id }
            | ty::AliasConstKind::Anon { def_id } => def_id,
        };

        self.services()
            .const_eval_inputs()
            .evaluation(ConstEvalQuery {
                typing_env,
                definition: def_id,
                constant: ct,
                span,
            })
    }
}

/// Supplemental control over a non-alias constant encoded by the actual source
/// owner. This helper creates no value, alias, provider or guest expectation.
#[cfg(test)]
pub(crate) fn check_actual_non_alias<'db>(
    infcx: &InferenceContext<'db>,
    param_env: ParamEnv<'db>,
    value: Const<'db>,
) {
    // The compiler client resolves inference before inspecting ConstKind.
    let resolved = infcx.deeply_resolve_ignoring_regions(value);
    assert!(
        !matches!(
            resolved.kind(),
            ty::ConstKind::Alias(..) | ty::ConstKind::Error(_)
        ),
        "this source control requires an actual non-alias, non-error constant"
    );
    let mut normalization_calls = 0;
    let result = try_evaluate_const(infcx, value, param_env, |value_type| {
        normalization_calls += 1;
        Ok::<_, ()>(value_type.skip_norm_wip())
    });
    match resolved.kind() {
        ty::ConstKind::Value(_) => assert_eq!(
            result.expect("a concrete source value is already evaluated upstream"),
            resolved
        ),
        ty::ConstKind::Param(_)
        | ty::ConstKind::Infer(_)
        | ty::ConstKind::Bound(..)
        | ty::ConstKind::Placeholder(_)
        | ty::ConstKind::Expr(_) => {
            assert!(
                matches!(result, Err(EvaluateConstErr::HasGenericsOrInfers)),
                "the pinned non-alias symbolic branch postpones evaluation"
            );
        }
        ty::ConstKind::Alias(..) | ty::ConstKind::Error(_) => {
            unreachable!("source-control input checked above")
        }
    }
    assert_eq!(
        normalization_calls, 0,
        "pinned non-alias branches never request Type normalization"
    );
}

/// Raw-data boundary check for a supplied actual owner definition. The empty
/// immutable table is not installed as a complete provider and evaluates no alias.
#[cfg(test)]
pub(crate) fn check_missing_owner_env(definition: DefId) {
    let inputs = PreparedConstEvalInputs::new(HashMap::new(), HashMap::new());
    assert_eq!(
        inputs.post_analysis_env(definition),
        Err(ConstEvalDependency::PostAnalysisEnv(definition))
    );
}

#!/usr/bin/env python3
"""Audit/regenerate pinned next-solver constant-normalization client.

Terminal compiler CTFE results are immutable prepared inputs, never an evaluator
callback. The outer Result keeps missing preparation distinct from upstream
semantic outcomes. All algorithm/representation transformations are below.
"""
import argparse
import hashlib
import re
from pathlib import Path
import subprocess
from sync_upstream_binders import method

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
PINS = {
    'compiler/rustc_trait_selection/src/solve/delegate.rs': 'ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095',
    'compiler/rustc_trait_selection/src/traits/mod.rs': '49a807c38f798fd55fbe908399114f6707ff480d66bfacc1416ddecc313d5cc8',
    'compiler/rustc_infer/src/infer/mod.rs': '0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327',
    'compiler/rustc_middle/src/ty/mod.rs': 'bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7',
    'compiler/rustc_ty_utils/src/ty.rs': '0d1edae3ada2b3186448a44c151a6d07916461b781f1803edb9f1e5f6bd5afee',
    'compiler/rustc_middle/src/mir/interpret/queries.rs': 'a56f205b0cdef592e53936b1d912af058231c04f81f4f89761247b39167c98f4',
}
TARGET = ROOT / 'crates/lrr/src/traits/upstream/infer/const_eval.rs'


def original(name):
    raw = (ROOT / 'vendor/rustc-trait-solver' / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != PINS[name]:
        raise ValueError('pinned const-evaluation source differs: ' + name)
    return raw.decode()


def selected(source, name):
    text = ('\n' + source).replace('\npub fn ', '\n    pub fn ').replace('\nfn ', '\n    pub fn ')
    text = text.replace('\n    fn ', '\n    pub fn ')
    return method(text, name)


def adapt(text):
    text = text.replace("'tcx", "'db").replace("TyCtxt<'db>", "DbInterner<'db>")
    text = text.replace("InferCtxt<'db>", "InferenceContext<'db>")
    text = text.replace("ty::Const<'db>", "Const<'db>").replace("ty::ParamEnv<'db>", "ParamEnv<'db>")
    text = text.replace("ty::AliasConst<'db>", "ty::AliasConst<DbInterner<'db>>")
    text = text.replace("Unnormalized<'db, Ty<'db>>", "ty::Unnormalized<DbInterner<'db>, Ty<'db>>")
    text = text.replace("GenericArgsRef<'db>", "Args<'db>")
    text = text.replace('GenericArgs::identity_for_item', 'Args::identity_for_item')
    text = text.replace('infcx.tcx', 'infcx.interner')
    text = text.replace('ty::Infer(', 'ty::TyKind::Infer(')
    text = text.replace('ErrorGuaranteed', 'ErrorToken')
    text = text.replace('tcx.thir_abstract_const(def_id)', 'tcx.services().abstract_const(tcx, def_id)')
    text = text.replace('tcx.dcx().delayed_bug(', 'tcx.delay_bug(')
    text = text.replace('ErrorHandled::', 'ConstEvalError::')
    text = text.replace('            use rustc_middle::mir::interpret::ErrorHandled;\n', '')
    text = text.replace('ty::Const::new_value(tcx, val, ty)', 'ty::Const::new(tcx, ty::ConstKind::Value(model::ValueConst { ty, tree: val }))')
    text = re.sub(r'\bbug!\(', 'panic!(', text.replace('ty::ty::', 'ty::'))
    return text


def expected():
    delegate = original('compiler/rustc_trait_selection/src/solve/delegate.rs')
    traits = original('compiler/rustc_trait_selection/src/traits/mod.rs')
    infer = original('compiler/rustc_infer/src/infer/mod.rs')
    types = original('compiler/rustc_middle/src/ty/mod.rs')
    utils = original('compiler/rustc_ty_utils/src/ty.rs')
    queries = original('compiler/rustc_middle/src/mir/interpret/queries.rs')

    # All upstream outcomes are unchanged; only an outer admission Result is
    # added because immutable terminal query leaves may still require input.
    outer = selected(delegate, 'evaluate_const')
    outer = outer.replace('    pub fn evaluate_const<E: Debug>(\n        &self,', "pub(crate) fn evaluate_const<'db, E: Debug>(\n        infcx: &InferenceContext<'db>,")
    outer = outer.replace("Result<Option<ty::Const<'tcx>>, E>", "Result<Result<Option<ty::Const<'tcx>>, E>, ConstEvalDependency<'tcx>>")
    outer = outer.replace('self.tcx', 'infcx.interner').replace('&self.0', 'infcx')
    outer = outer.replace('crate::traits::try_evaluate_const', 'try_evaluate_const')
    outer = outer.replace('Ok(ct) => Ok(Some(ct)),', 'Ok(ct) => Ok(Ok(Some(ct))),')
    outer = outer.replace('Ok(Some(ty::Const::new_error(infcx.interner, e)))', 'Ok(Ok(Some(ty::Const::new_error(infcx.interner, e))))')
    outer = outer.replace('=> Ok(None),', '=> Ok(Ok(None)),')
    outer = outer.replace('Err(EvaluateConstErr::FailedNormalization(e)) => Err(e),', 'Err(EvaluateConstErr::FailedNormalization(e)) => Ok(Err(e)),\n            Err(EvaluateConstErr::Preparation(dependency)) => Err(dependency),')
    outer = adapt(outer)

    start = traits.index('pub enum EvaluateConstErr<E>')
    end = traits.index('\n}', start) + 2
    errors = '#[derive(Debug)]\n' + traits[start:end]
    errors = errors.replace('pub enum EvaluateConstErr<E>', "enum EvaluateConstErr<'db, E>")
    errors = errors.replace('    FailedNormalization(E),', "    FailedNormalization(E),\n    /// Required prepared input was not admitted; never a semantic failure.\n    Preparation(ConstEvalDependency<'db>),")
    errors = adapt(errors)

    evaluate = adapt(selected(traits, 'try_evaluate_const'))
    evaluate = evaluate.replace('    pub fn ', 'fn ', 1)
    evaluate = evaluate.replace('EvaluateConstErr<E>', "EvaluateConstErr<'db, E>")
    evaluate = evaluate.replace('ty::TypingEnv::', 'TypingEnv::')
    evaluate = evaluate.replace('TypingEnv::post_analysis(tcx, def_id);', 'TypingEnv::post_analysis(tcx, def_id).map_err(EvaluateConstErr::Preparation)?;')
    evaluate = evaluate.replace('TypingEnv::fully_monomorphized()', 'TypingEnv::fully_monomorphized(tcx)')
    evaluate = evaluate.replace('match tcx.const_eval_resolve_for_typeck(typing_env, erased_alias_const, span) {', 'match tcx.const_eval_resolve_for_typeck(typing_env, erased_alias_const, span).map_err(EvaluateConstErr::Preparation)? {')

    folder = adapt(selected(traits, 'replace_param_and_infer_args_with_placeholder'))
    folder = folder.replace('    pub fn ', 'fn ', 1)

    typing_env = selected(infer, 'typing_env')
    typing_env = adapt(typing_env).replace("ty::TypingEnv<'db>", "TypingEnv<'db>")
    typing_env = typing_env.replace('ty::TypingEnv::new', 'TypingEnv::new')
    typing_impl = "impl<'db> InferenceContext<'db> {\n" + typing_env + '}\n'

    # Reuse the actual shared TypingModeEqWrapper; do not invent Eq semantics.
    start = types.index("pub struct TypingEnv<'tcx>")
    end = types.index('\n}', start) + 2
    env_struct = types[start:end]
    env_struct = env_struct.replace('    #[type_foldable(identity)]\n', '').replace('    #[type_visitable(ignore)]\n', '')
    env_struct = adapt(env_struct).replace('pub struct ', 'pub(crate) struct ')
    env_struct = env_struct.replace("TypingModeEqWrapper<'db>", "ty::TypingModeEqWrapper<DbInterner<'db>>")
    env_struct = '#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]\n' + env_struct

    # There are many new() functions in ty/mod.rs; restrict to TypingEnv impl.
    env_source = types[types.index("impl<'tcx> TypingEnv<'tcx> {"):]
    env_methods = ''.join(selected(env_source, name) for name in ('new', 'fully_monomorphized', 'post_analysis', 'with_post_analysis_normalized'))
    env_methods = adapt(env_methods).replace("ty::TypingEnv<'db>", "TypingEnv<'db>")
    env_methods = env_methods.replace("TypingMode<'db>", "ty::TypingMode<DbInterner<'db>>")
    env_methods = env_methods.replace('TypingModeEqWrapper(typing_mode)', 'ty::TypingModeEqWrapper(typing_mode)')
    env_methods = env_methods.replace('pub fn fully_monomorphized()', "pub fn fully_monomorphized(tcx: DbInterner<'db>)")
    env_methods = env_methods.replace('ParamEnv::empty()', 'model::ParamEnv(tcx.clauses(model::List::default()))')
    env_methods = env_methods.replace('def_id: impl IntoQueryKey<DefId>', 'def_id: DefId')
    env_methods = env_methods.replace("pub fn post_analysis(tcx: DbInterner<'db>, def_id: DefId) -> TypingEnv<'db>", "pub fn post_analysis(tcx: DbInterner<'db>, def_id: DefId) -> Result<TypingEnv<'db>, ConstEvalDependency<'db>>")
    env_methods = env_methods.replace('TypingEnv::new(tcx.param_env_normalized_for_post_analysis(def_id), TypingMode::PostAnalysis)', 'Ok(TypingEnv::new(param_env_normalized_for_post_analysis(tcx, def_id)?, TypingMode::PostAnalysis))')
    # Pinned ParamEnv::with_normalized returns self for the required next solver.
    # Keep the mode's exhaustive match, eliminating only its legacy query branch.
    env_methods = env_methods.replace('let param_env = param_env.with_normalized(tcx);', 'let param_env = param_env;')
    env_methods = env_methods.replace("pub fn with_post_analysis_normalized(self, tcx: DbInterner<'db>)", "pub fn with_post_analysis_normalized(self, _tcx: DbInterner<'db>)")
    env_impl = "impl<'db> TypingEnv<'db> {\n" + env_methods + '}\n'

    owner_env = selected(utils, 'param_env_normalized_for_post_analysis')
    owner_env = owner_env.replace("fn param_env_normalized_for_post_analysis(tcx: TyCtxt<'_>, def_id: DefId) -> ty::ParamEnv<'_>", "fn param_env_normalized_for_post_analysis<'db>(tcx: DbInterner<'db>, def_id: DefId) -> Result<ParamEnv<'db>, ConstEvalDependency<'db>>")
    owner_env = owner_env.replace('    pub fn ', 'fn ', 1)
    owner_env = owner_env.replace('tcx.param_env(def_id).with_normalized(tcx)', 'tcx.services().const_eval_inputs().post_analysis_env(def_id)')

    # Retain all compiler preconditions before its instance-resolution/CTFE
    # infrastructure boundary. Prepared query results carry the same complete
    # success/non-valtree/reported/too-generic outcome algebra; diagnostic lint
    # inspection after successful CTFE is static tooling outside E-R5-006.
    terminal = adapt(selected(queries, 'const_eval_resolve_for_typeck'))
    terminal = terminal[:terminal.index('        let cid = match')]
    terminal = terminal.replace("ty::TypingEnv<'db>", "TypingEnv<'db>")
    terminal = terminal.replace("ConstToValTreeResult<'db>", "Result<ConstEvalResult<'db>, ConstEvalDependency<'db>>")
    terminal += '''        self.services().const_eval_inputs().evaluation(ConstEvalQuery {
            typing_env, definition: def_id, constant: ct, span,
        })
    }
'''
    terminal_impl = "impl<'db> DbInterner<'db> {\n" + terminal + '}\n'
    text = PRELUDE + env_struct + '\n' + BOUNDARY + '\n' + errors + '\n' + outer + evaluate + folder + typing_impl + env_impl + owner_env + terminal_impl + CONTROLS
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(), capture_output=True, check=True).stdout


PRELUDE = '''//! Pinned next-solver constant normalization; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_const_eval.py.
//! Compiler CTFE query leaves are typed immutable preparation inputs, not a
//! callback evaluator. Missing leaves are explicit admission dependencies.
use super::{InferenceContext, DbInterner, Const, Ty, Span, PreparedProvider};
use crate::traits::upstream::{facts::SourceFacts as _, identity::DefinitionId as DefId,
    model::{self, ErrorToken}};
use rustc_type_ir::{self as ty, inherent::*, Interner, TypeFoldable, TypeSuperFoldable,
    TypeVisitableExt, TypeFolder, TypingMode, MayBeErased};
use rustc_type_ir::inherent::Ty as _;
use std::{collections::HashMap, fmt::Debug};
use tracing::debug;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type Args<'db> = model::Args<'db, PreparedProvider<'db>>;
type ValTree<'db> = model::ValTree<'db, PreparedProvider<'db>>;

'''

BOUNDARY = '''
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
        Self { post_analysis_envs, evaluations }
    }
    pub(crate) fn post_analysis_env(&self, definition: DefId) -> Result<ParamEnv<'db>, ConstEvalDependency<'db>> {
        self.post_analysis_envs.get(&definition).copied().ok_or(ConstEvalDependency::PostAnalysisEnv(definition))
    }
    pub(crate) fn evaluation(&self, query: ConstEvalQuery<'db>) -> Result<ConstEvalResult<'db>, ConstEvalDependency<'db>> {
        self.evaluations.get(&query).copied().ok_or(ConstEvalDependency::Evaluation(query))
    }
}

'''



CONTROLS = '''
/// Supplemental control over a non-alias constant encoded by the actual source
/// owner. This helper creates no value, alias, provider or guest expectation.
#[cfg(test)]
pub(crate) fn check_actual_non_alias<'db>(infcx: &InferenceContext<'db>, param_env: ParamEnv<'db>, value: Const<'db>) {
    // The compiler client resolves inference before inspecting ConstKind.
    let resolved = infcx.deeply_resolve_ignoring_regions(value);
    assert!(!matches!(resolved.kind(), ty::ConstKind::Alias(..) | ty::ConstKind::Error(_)),
        "this source control requires an actual non-alias, non-error constant");
    let mut normalization_calls = 0;
    let result = try_evaluate_const(infcx, value, param_env, |value_type| {
        normalization_calls += 1;
        Ok::<_, ()>(value_type.skip_norm_wip())
    });
    match resolved.kind() {
        ty::ConstKind::Value(_) => assert_eq!(result.expect("a concrete source value is already evaluated upstream"), resolved),
        ty::ConstKind::Param(_) | ty::ConstKind::Infer(_) | ty::ConstKind::Bound(..)
        | ty::ConstKind::Placeholder(_) | ty::ConstKind::Expr(_) => {
            assert!(matches!(result, Err(EvaluateConstErr::HasGenericsOrInfers)),
                "the pinned non-alias symbolic branch postpones evaluation");
        }
        ty::ConstKind::Alias(..) | ty::ConstKind::Error(_) => unreachable!("source-control input checked above"),
    }
    assert_eq!(normalization_calls, 0, "pinned non-alias branches never request Type normalization");
}

/// Raw-data boundary check for a supplied actual owner definition. The empty
/// immutable table is not installed as a complete provider and evaluates no alias.
#[cfg(test)]
pub(crate) fn check_missing_owner_env(definition: DefId) {
    let inputs = PreparedConstEvalInputs::new(HashMap::new(), HashMap::new());
    assert_eq!(inputs.post_analysis_env(definition), Err(ConstEvalDependency::PostAnalysisEnv(definition)));
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    data = expected()
    if args.write:
        TARGET.write_bytes(data)
    elif TARGET.read_bytes() != data:
        raise ValueError('constant client differs from registered pinned transformation')
    print('Pinned constant client hashes and registered transformation match; actual terminal producer/admission/root/runtime remain held.')


if __name__ == '__main__':
    main()

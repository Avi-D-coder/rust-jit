"""Registered pinned compiler region storage/undo and forall client projection.

Called by sync_upstream_inference.py. No new formula, transaction, binder walker,
leak checker or solver algorithm. All selected method bodies remain upstream.
"""
import hashlib
from pathlib import Path
from sync_upstream_binders import method
from sync_upstream_inference_context import adapt

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'vendor/rustc-trait-solver'
BASE = 'compiler/rustc_infer/src/infer/'
HASHES = {
    'mod.rs': '0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327',
    'solver_region_constraints.rs': '54d9d1f2af43c310ad4c0974506ae29bc6c988b7d1990371100e045408110b63',
    'solver_region_constraints/tests.rs': '2fef381b0f32fc297062afaf413bb78cce4f4f94391e17715add1acca0ba949a',
    'snapshot/undo_log.rs': '3a39097df268547440ada1daac05ed4eeace02b1655d9fd4def77f1bd90fa5fa',
    'outlives/obligations.rs': '30b0ed80442caa513768a5b25a28310d3c37370d3f1f588cd3e3a6f210ca2dae',
    'relate/higher_ranked.rs': 'ff3ebc86ca7ced60b52a92d3c761ed12e72ea6a233f69ae2b1fde0afae159589',
}


def original(name):
    raw = (SUPPORT / (BASE + name)).read_bytes()
    if hashlib.sha256(raw).hexdigest() != HASHES[name]:
        raise ValueError('solver region client original differs: ' + name)
    return raw.decode()


def project(text):
    text = adapt(text)
    for name in ('Region', 'BoundRegion', 'BoundTy', 'BoundConst'):
        text = text.replace('ty::' + name + "<'db>", 'ty::' + name + "<DbInterner<'db>>")
    text = text.replace("ty::Binder<'db,", "ty::Binder<DbInterner<'db>,")
    return text


def extend_storage(outputs, header, rustfmt):
    compiler = original('mod.rs')
    source = original('solver_region_constraints.rs')
    source = source[:source.index('#[cfg(test)]')]
    source = source.replace('use rustc_middle::ty::TyCtxt;\nuse rustc_span::Span;',
                            'use super::{DbInterner, Span};')
    source = source.replace("TyCtxt<'tcx>", "DbInterner<'db>").replace("'tcx", "'db")
    clients = ''.join(method(compiler, n) for n in (
        'get_solver_region_constraint', 'overwrite_solver_region_constraint'))
    clients += method(original('outlives/obligations.rs'), 'register_solver_region_constraint')
    outputs['solver_region_constraints.rs'] = rustfmt(header + source + '''
use super::InferenceContext;
use super::snapshot::undo_log::UndoLog;
use ena::undo_log::UndoLogs;
use rustc_type_ir::TypeVisitableExt;
impl<'db> InferenceContext<'db> {
''' + project(clients) + '}\n')
    # Extend the existing one storage owner with the actual compiler field/init.
    field = next(line for line in compiler.splitlines() if line.startswith('    solver_region_constraint_storage:'))
    init = next(line for line in compiler.splitlines() if line.startswith('            solver_region_constraint_storage:'))
    body = outputs['mod.rs'].decode()
    body = body.replace('pub(crate) mod snapshot;', '''pub(crate) mod snapshot;
pub(crate) mod solver_region_constraints;
mod higher_ranked;
use solver_region_constraints::SolverRegionConstraintStorage;''', 1)
    body = body.replace('    /// A set of constraints that regionck must validate.',
                        '    pub(crate) ' + project(field.strip()) + '\n\n    /// A set of constraints that regionck must validate.', 1)
    body = body.replace('            region_obligations: vec![],', project(init) + '\n            region_obligations: vec![],', 1)
    outputs['mod.rs'] = rustfmt(body)
    undo = original('snapshot/undo_log.rs')
    variant = next(line for line in undo.splitlines() if line.startswith('    OverwriteSolverRegionConstraint {'))
    start = undo.index('            UndoLog::OverwriteSolverRegionConstraint {')
    stop = undo.index('            UndoLog::PushTypeOutlivesConstraint =>', start)
    reverse = undo[start:stop]
    body = outputs['snapshot/undo_log.rs'].decode()
    body = body.replace('use tracing::debug;', 'use tracing::debug;\nuse super::super::solver_region_constraints::SolverRegionConstraint;', 1)
    body = body.replace('    PushTypeOutlivesConstraint,', project(variant) + '\n    PushTypeOutlivesConstraint,', 1)
    body = body.replace('            UndoLog::PushTypeOutlivesConstraint =>', project(reverse) + '            UndoLog::PushTypeOutlivesConstraint =>', 1)
    outputs['snapshot/undo_log.rs'] = rustfmt(body)
    forall = ''.join(method(original('relate/higher_ranked.rs'), name)
                     for name in ('enter_forall_and_leak_universe', 'enter_forall'))
    outputs['higher_ranked.rs'] = rustfmt(header + '''use super::{InferenceContext, DbInterner, Ty};
use crate::traits::upstream::binders::FnMutDelegate;
use rustc_type_ir::{self as ty, TypeFoldable, TypeVisitableExt, inherent::Ty as _};
use tracing::debug;
impl<'db> InferenceContext<'db> {
''' + project(forall) + '}\n')
    tests = original('solver_region_constraints/tests.rs')
    # Retain the original unit test unchanged. Supply actual source context/spans
    # outside it, then run its exact assertion body with only the IR type alias.
    test_body = tests[tests.index('    let first = LeafRegionConstraint::'):tests.rindex('}')]
    test_body = test_body.replace("TyCtxt<'_>", "DbInterner<'db>")
    outputs['solver_region_constraints_tests.rs'] = rustfmt(header + '''use super::{DbInterner, Span};
use rustc_type_ir::region_constraint::{And, LeafRegionConstraint, Or};
pub(super) fn canonicalization_preserves_only_one_ambiguity<'db>(
    _cx: DbInterner<'db>, first: Span, second: Span,
) {
''' + test_body + '}\n')
    outputs['mod.rs'] += b'\n#[cfg(test)]\nmod solver_region_constraints_tests;\n'

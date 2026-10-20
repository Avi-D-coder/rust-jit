#!/usr/bin/env python3
"""Audit/regenerate pinned next-solver inference storage; no custom transactions.

Only concrete IR/import/diagnostic-span spelling changes. The original licensed
files stay unchanged. Actual complete source/query-input production and root admission remain held.
"""
import argparse
import hashlib
from pathlib import Path
from sync_upstream_binders import method
import subprocess
from upstream_solver_regions import extend_storage
from upstream_region_collector import extend_collector
from upstream_leak_check import extend_leak_check
from upstream_obligation_state import extend_obligation_state
from upstream_infer_client import extend_infer_client
from upstream_canonical_instantiation import extend_canonical_instantiation
from upstream_region_response import extend_region_response
from upstream_goal_fast_path import extend_goal_fast_path
from upstream_solver_delegate import extend_solver_delegate
from upstream_hidden_type_bounds import extend_hidden_type_bounds
from upstream_wellformed import expected as expected_well_formed
from upstream_const_eval import expected as expected_const_eval
from upstream_transmute import adapter as expected_transmute

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
BASE = 'src/tools/rust-analyzer/crates/hir-ty/src/'
SUPPORT = ROOT / 'vendor/rustc-trait-solver'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/infer'
# Exact pinned Git-object hashes, independent of the source inventory.
HASHES = {'lib.rs': '02d3b83996bbbf175a6aede097e451036c75e66bc39fe5982bc7bed1056a4a6b', 'next_solver/infer/mod.rs': '1880e80e62a6eff52160d948398874c2bd0453d3980d003a816725e0a2395d37', 'next_solver/infer/type_variable.rs': 'e9c000e183d3ecef94785a7f841b78e85ad97f716f373d4f75c309a84fcd42f1', 'next_solver/infer/unify_key.rs': '9c78803d35123de368bfc93b41616f9630b82530b6c931f3ba174c515d7163a8', 'next_solver/infer/region_constraints/mod.rs': 'a49d71d15a9a8db3304b2647f2e00c0bc36c5535e9dc7a4f54ba49a109b4a155', 'next_solver/infer/snapshot/undo_log.rs': '14af652f6ce6506b615f2a03241b55c1d1413bc5b6263fb5b345c5d09f373cea', 'next_solver/infer/opaque_types/mod.rs': 'c64b0d57ec2f4b343f064e4725286a896f6a6df94c132c4b9734d56e7e30f621', 'next_solver/infer/opaque_types/table.rs': '15937251caffd2a44f1f93630ff6bc31db2e5a5ca0d3a97f9d3d257044c8d7a3'}
HASHES['next_solver/infer/snapshot/mod.rs'] = '8ffc82602d8978b8afe0e507f7f585fa8a247b9b7b5696af1624088838ca9f75'


def original(name):
    raw = (SUPPORT / (BASE + name)).read_bytes()
    if hashlib.sha256(raw).hexdigest() != HASHES[name]:
        raise ValueError('retained inference source differs from pinned Git object: ' + name)
    return raw.decode()


def rustfmt(text):
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(),
                          check=True, capture_output=True).stdout


def adapt(text):
    text = text.replace('crate::next_solver::infer', 'crate::traits::upstream::infer')
    text = text.replace('crate::next_solver', 'crate::traits::upstream::infer')
    text = text.replace('    next_solver::infer::', '    traits::upstream::infer::')
    text = text.replace('    next_solver::{', '    traits::upstream::infer::{')
    text = text.replace('    infer::snapshot::', '    snapshot::')
    text = text.replace('use crate::Span;', 'use super::Span;')
    text = text.replace('    Span,\n', '    traits::upstream::model::Span,\n')
    text = text.replace('use rustc_type_ir::inherent::IntoKind;',
                        'use rustc_type_ir::inherent::{IntoKind, Ty as _};')
    # RA wraps these source/alias constructors; use the actual shared IR APIs.
    text = text.replace('(*p).to_ty(interner)', 'Ty::new_param(interner, *p)')
    text = text.replace('p.to_ty(interner)', 'p.to_ty(interner, rustc_type_ir::IsRigid::Yes)')
    text = text.replace('GenericKind::Alias(ref p) => Ty::new_param(interner, *p)', 'GenericKind::Alias(ref p) => p.to_ty(interner, rustc_type_ir::IsRigid::Yes)')
    # LRR spans carry actual source/range; the copied preference is unchanged.
    return text


def expected():
    header = ('//! Pinned next-solver inference storage client; MIT OR Apache-2.0.\n'
              f'//! Pin {PIN}; originals/licenses in vendor/rustc-trait-solver.\n'
              '//! Registered mechanical port: tools/sync_upstream_inference.py.\n')
    outputs = {}
    for name in ('type_variable.rs', 'unify_key.rs', 'region_constraints/mod.rs',
                 'snapshot/undo_log.rs', 'opaque_types/table.rs'):
        text = original('next_solver/infer/' + name)
        outputs[name] = rustfmt(header + adapt(text))
    outputs['region_constraints/mod.rs'] = extend_collector(outputs['region_constraints/mod.rs'].decode(), rustfmt)
    source = original('next_solver/infer/mod.rs')
    table = source[source.index('pub(crate) type UnificationTable'):source.index('/// This type contains all the things')]
    inner = source[source.index('#[derive(Clone)]\npub struct InferCtxtInner'):source.index('#[derive(Clone)]\npub struct InferCtxt<')]
    outlives = source[source.index('#[derive(Clone, Debug)]\npub struct TypeOutlivesConstraint'):source.index('/// Used to configure inference contexts')]
    member = source[source.index('#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct MemberConstraint'):]
    imports = '''use std::{ops::Range, sync::Arc, cell::{Cell, RefCell}};
use ena::unify as ut;
use rustc_type_ir::{TyVid, IntVid, FloatVid, UniverseIndex};
use tracing::debug;
use opaque_types::OpaqueTypeStorage;
use region_constraints::{RegionConstraintStorage, RegionConstraintCollector};
use unify_key::ConstVidKey;
pub(crate) use super::{prepared::PreparedProvider, model::Span};
pub(crate) type DbInterner<'db> = super::context::Context<'db, PreparedProvider<'db>>;
pub(crate) type Ty<'db> = super::model::Ty<'db, PreparedProvider<'db>>;
pub(crate) type Const<'db> = rustc_type_ir::Const<DbInterner<'db>>;
pub(crate) type Region<'db> = rustc_type_ir::Region<DbInterner<'db>>;
pub(crate) type Binder<'db, T> = rustc_type_ir::Binder<DbInterner<'db>, T>;
pub(crate) type AliasTy<'db> = rustc_type_ir::AliasTy<DbInterner<'db>>;
pub(crate) type PlaceholderType<'db> = rustc_type_ir::PlaceholderType<DbInterner<'db>>;
pub(crate) type ParamTy = super::parameters::SourceParameter;
pub(crate) type OpaqueTypeKey<'db> = rustc_type_ir::OpaqueTypeKey<DbInterner<'db>>;
pub(crate) type ArgOutlivesPredicate<'db> = rustc_type_ir::OutlivesClause<DbInterner<'db>, super::model::Arg<'db, PreparedProvider<'db>>>;
pub(crate) type FxIndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
pub(crate) mod type_variable;
pub(crate) mod unify_key;
pub(crate) mod region_constraints;
pub(crate) mod opaque_types;
pub(crate) mod snapshot;
/// Mutable storage only; this does not implement InferCtxtLike or SolverDelegate.
pub(crate) struct InferenceStorage<'db> {
    pub(crate) inner: RefCell<InferCtxtInner<'db>>,
    universe: Cell<UniverseIndex>,
}
impl<'db> InferenceStorage<'db> {
    pub(crate) fn new() -> Self {
        Self { inner: RefCell::new(InferCtxtInner::new()), universe: Cell::new(UniverseIndex::ROOT) }
    }
}
pub(crate) use snapshot::undo_log::InferCtxtUndoLogs;
#[cfg(test)] pub(crate) mod tests;
'''
    outputs['mod.rs'] = rustfmt(header + imports + table + inner + outlives + member)
    opaque = original('next_solver/infer/opaque_types/mod.rs')
    record = opaque[opaque.index('#[derive(Copy, Clone, Debug, TypeVisitable, TypeFoldable)]'):opaque.index("impl<'db> InferCtxt")]
    record = record.replace(', TypeVisitable, TypeFoldable', '')
    snapshots = original('next_solver/infer/snapshot/mod.rs').replace('mod fudge;\n', '').replace('InferCtxt', 'InferenceStorage')
    outputs['snapshot/mod.rs'] = rustfmt(header + snapshots)
    universes = method(source, 'universe') + method(source, 'create_next_universe')
    outputs['mod.rs'] += rustfmt("impl<'db> InferenceStorage<'db> {\n" + universes + '}\n')
    outputs['opaque_types/mod.rs'] = rustfmt(header + '''pub(crate) mod table;
pub(crate) use table::{OpaqueTypeStorage, OpaqueTypeTable};
use super::Ty;
''' + record)
    # Only the span-selection operation, never RA expression/body identity.
    spans = original('lib.rs')
    start = spans.index('    pub(crate) fn pick_best(a: Span, b: Span)')
    end = spans.index('    #[inline]', start)
    pick = spans[start:end]
    outputs['span.rs'] = rustfmt(header + '''use super::Span;
impl Span {
''' + pick + '''pub(crate) fn is_dummy(&self) -> bool {
    self.origin == crate::core::digest::Digest::default() && self.start == 0 && self.end == 0
}
}
''')
    # The span representation implements the pinned dummy-span preference.
    outputs['mod.rs'] = rustfmt(outputs['mod.rs'].decode().replace('pub(crate) mod type_variable;', 'mod span;\npub(crate) mod type_variable;'))
    outputs['type_variable.rs'] += b'\ninclude!(\"root_probe.rs\");\n'
    outputs['mod.rs'] += b'\nmod context;\npub(crate) use context::InferenceContext;\nmod variables;\nmod resolve;\n'
    extend_storage(outputs, header, rustfmt)
    extend_leak_check(outputs, header, rustfmt)
    extend_obligation_state(outputs, header, rustfmt)
    extend_infer_client(outputs, header, rustfmt)
    extend_canonical_instantiation(outputs, header, rustfmt)
    extend_region_response(outputs, header, rustfmt)
    extend_goal_fast_path(outputs, header, rustfmt)
    extend_solver_delegate(outputs, header, rustfmt)
    extend_hidden_type_bounds(outputs, header, rustfmt)
    outputs['well_formed.rs'] = expected_well_formed()
    outputs['const_eval.rs'] = expected_const_eval()
    outputs['transmute.rs'] = expected_transmute()
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        for name, raw in expected().items():
            target = TARGET / name
            if args.write:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(raw)
            if target.read_bytes() != raw:
                raise ValueError('inference storage differs from registered pinned client: ' + name)
        print('Pinned inference storage: exact originals and registered mechanical port; no local undo/commit/unification algorithm; actual source/query admission remains required.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n' + getattr(error, 'stderr', b'').decode(errors='replace'))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Audit/register pinned inference variable/resolution/binder client operations.

No generic relation, trait solver or canonicalizer is reimplemented here. Exact
originals and their registered concrete IR adapters are checked independently.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
from sync_upstream_binders import method

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
SUPPORT = ROOT / 'vendor/rustc-trait-solver'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/infer'
HASHES = {'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/mod.rs': '1880e80e62a6eff52160d948398874c2bd0453d3980d003a816725e0a2395d37', 'compiler/rustc_infer/src/infer/mod.rs': '0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327', 'compiler/rustc_infer/src/infer/type_variable.rs': '2e5f26dba08abf4f96c52975f1ac0fba42c37d2e650de96f9a182314e7b88f53', 'compiler/rustc_infer/src/infer/resolve.rs': '7ce8d8532e4357552be548f1a66e81b3c89aff0c8f934785c0492daa6282dcac'}
RA = 'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/mod.rs'
COMPILER = 'compiler/rustc_infer/src/infer/mod.rs'

HASHES['compiler/rustc_middle/src/ty/generic_args.rs'] = '83f30406c92c7e2d92f8c2b439e37679fc9d5a33519bf3591e02932ec0250279'

def original(name):
    raw = (SUPPORT / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != HASHES[name]:
        raise ValueError('inference client original differs: ' + name)
    return raw.decode()


def fmt(text):
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(),
                          check=True, capture_output=True).stdout


def adapt(text):
    for name in ('Ty', 'Const', 'Term', 'Clauses'):
        text = text.replace('ty::' + name + "<'tcx>", name + "<'db>")
    text = text.replace("Ty<'tcx>", "Ty<'db>")
    text = text.replace("TyCtxt<'tcx>", "DbInterner<'db>")
    text = text.replace("'tcx", "'db").replace('self.tcx', 'self.interner').replace('self.infcx.tcx', 'self.infcx.interner')
    text = text.replace('InferCtxt', 'InferenceContext')
    text = text.replace('*ty.kind()', 'ty.kind()')
    text = text.replace('ConstKind::Unevaluated(_)', 'ConstKind::Alias(_, _)')
    text = text.replace('ConstVariableValue::Unknown { origin: _, universe }', 'ConstVariableValue::Unknown { span: _, universe }')
    text = text.replace("BoundVarReplacerDelegate<'db>", "BoundVarReplacerDelegate<'db, PreparedProvider<'db>>")
    text = text.replace('BoundVariableKind::Ty(_)', 'BoundVariableKind::Ty(..)')
    for name, kind in (('new_int', 'Int'), ('new_uint', 'Uint'), ('new_float', 'Float')):
        text = text.replace('Ty::' + name + '(self.interner, ty)', 'self.interner.ty(ty::TyKind::' + kind + '(ty))')
    for name, kind, args in (('new_int_var', 'IntVar', ('vid', 'root', 'inner.int_unification_table().find(vid)')), ('new_float_var', 'FloatVar', ('vid', 'root', 'self.next_float_vid()', 'inner.float_unification_table().find(vid)'))):
        for arg in args:
            text = text.replace('Ty::' + name + '(self.interner, ' + arg + ')', 'Ty::new_infer(self.interner, ty::InferTy::' + kind + '(' + arg + '))')
    return text


def expected():
    header = ('//! Pinned inference variable/resolution client; MIT OR Apache-2.0.\n'
              f'//! Pin {PIN}; originals/licenses in vendor/rustc-trait-solver.\n'
              '//! Registered mechanical port: tools/sync_upstream_inference_context.py.\n')
    ra = original(RA); compiler = original(COMPILER)
    ra_names = ('num_ty_vars', 'next_ty_var', 'next_ty_vid', 'next_ty_var_id_in_universe',
        'next_ty_var_in_universe', 'next_const_var', 'next_const_vid', 'next_const_vid_in_universe',
        'next_const_var_in_universe', 'next_int_var', 'next_int_vid', 'next_float_var', 'next_float_vid',
        'next_region_var', 'next_region_vid', 'next_region_var_in_universe', 'next_term_var_of_kind',
        'universe_of_region', 'num_region_vars', 'tainted_by_errors', 'set_tainted_by_errors',
        'type_var_span', 'const_var_span', 'instantiate_binder_with_fresh_vars')
    compiler_names = ('try_resolve_ty_var', 'shallow_resolve_ty_var_or_get_root', 'shallow_resolve',
        'shallow_resolve_const', 'shallow_resolve_term', 'root_var', 'sub_unify_ty_vids_raw',
        'sub_unification_table_root_var', 'root_float_var', 'root_const_var', 'shallow_resolve_const_var',
        'shallow_resolve_ty_var', 'shallow_resolve_int_var', 'shallow_resolve_float_var',
        'deeply_resolve_ignoring_regions', 'try_resolve_const_var')
    imports = '''use super::{InferenceContext, DbInterner, Ty, Const, Region, Span, PreparedProvider};
use super::{type_variable, type_variable::TypeVariableValue, unify_key::ConstVariableValue, resolve};
use rustc_type_ir::inherent::{Ty as _, GenericArg as _};
use crate::traits::upstream::{model::{Term, Clauses, ErrorToken as ErrorGuaranteed}, binders::BoundVarReplacerDelegate};
use rustc_type_ir::{self as ty, Interner, TypeFoldable, TypeVisitableExt, TyVid, ConstVid, IntVid, FloatVid, RegionVid, UniverseIndex, IntVarValue, FloatVarValue, InferConst, TermKind, BoundVariableKind, BoundTy, BoundRegion, BoundConst, inherent::*};
use tracing::debug;
type GenericArg<'db> = crate::traits::upstream::model::Arg<'db, PreparedProvider<'db>>;
type Binder<'db, T> = rustc_type_ir::Binder<DbInterner<'db>, T>;
'''
    # Model Term/Clauses each carry the concrete SourceFacts family.
    imports = imports.replace('model::{Term, Clauses, ErrorToken as ErrorGuaranteed}', 'model::{ErrorToken as ErrorGuaranteed}')
    imports += '''type Term<'db> = crate::traits::upstream::model::Term<'db, PreparedProvider<'db>>;
type Clauses<'db> = crate::traits::upstream::model::Clauses<'db, PreparedProvider<'db>>;
type SolverDefId<'db> = crate::traits::upstream::identity::DefinitionId;
'''
    mode = ra[ra.index('#[derive(Clone, Copy, Debug)]\npub enum BoundRegionConversionTime'):ra.index('/// See the `region_obligations` field')]
    # The source ID is lifetime independent in LRR; retain the enum's actual data.
    mode = mode.replace("BoundRegionConversionTime<'db>", 'BoundRegionConversionTime').replace("SolverDefId<'db>", 'crate::traits::upstream::identity::DefinitionId')
    blocks = ''.join(method(ra, n) for n in ra_names)
    blocks = blocks.replace("BoundRegionConversionTime<'db>", 'BoundRegionConversionTime')
    blocks = blocks.replace("BoundRegion<'db>", "BoundRegion<DbInterner<'db>>").replace("BoundTy<'db>", "BoundTy<DbInterner<'db>>").replace("BoundConst<'db>", "BoundConst<DbInterner<'db>>")
    text = header + imports + mode + "impl<'db> InferenceContext<'db> {\n" + adapt(blocks + ''.join(method(compiler, n) for n in compiler_names)) + '}\n'
    result = {'variables.rs':fmt(text)}
    source = original('compiler/rustc_infer/src/infer/resolve.rs')
    start = source.index('pub struct DeepResolverIgnoringRegions')
    stop = source.index('///////////////////////////////////////////////////////////////////////////\n// FULL TYPE RESOLUTION', start)
    result['resolve.rs'] = fmt(header + '''use super::{InferenceContext, DbInterner, Ty, Const, PreparedProvider};
use rustc_type_ir::{self as ty, TypeFolder, TypeSuperFoldable, TypeVisitableExt, PredicateProxy, data_structures::DelayedMap};
type Clauses<'db> = crate::traits::upstream::model::Clauses<'db, PreparedProvider<'db>>;
''' + adapt(source[start:stop]))
    source = original('compiler/rustc_infer/src/infer/type_variable.rs')
    # These exact compiler accessors supply canonical roots to the newer resolver.
    blocks = source[source.index('    pub(crate) fn probe_with_root_vid'):source.index('    #[inline]\n    fn eq_relations', source.index('    pub(crate) fn probe_with_root_vid'))]
    blocks = blocks.replace('ty::TyVid', 'TyVid')
    result['root_probe.rs'] = fmt(header.replace('//!', '//') + "impl<'a, 'db> TypeVariableTable<'a, 'db> {\n" + adapt(blocks) + '}\n')
    source = original('compiler/rustc_middle/src/ty/generic_args.rs')
    constructor = method(source, 'for_item') + method(source, 'fill_single')
    for old, new in (
        ("<'tcx>", "<'cx>"), ('for_item<F>', 'for_item<Make>'), ('fill_single<F>', 'fill_single<Make>'),
        ('F: FnMut', 'Make: FnMut'), ('mut mk_kind: F', 'mut mk_kind: Make'), ('mk_kind: &mut F', 'mk_kind: &mut Make'),
        ("tcx: TyCtxt<'cx>", "cx: Context<'cx, F>"), ('def_id: DefId', 'def_id: DefinitionId'),
        ("GenericArgsRef<'cx>", 'Self'), ("GenericArg<'cx>", "Arg<'cx, F>"),
        ('&ty::GenericParamDef', "GenericParameter<'cx, F>"), ("&mut SmallVec<[Arg<'cx, F>; 8]>", "&mut Vec<Arg<'cx, F>>"),
        ('defs: &ty::Generics', "defs: Generics<'cx, F>"), ('tcx.generics_of', 'cx.generics_of'),
        ('SmallVec::with_capacity', 'Vec::with_capacity'), ('Self::fill_item(&mut args, tcx, defs', 'Self::fill_single(&mut args, defs'),
        ('tcx.mk_args(&args)', 'cx.mk_args_from_iter(args.into_iter())'),
        ('defs.own_params.len()', 'defs.parameters.items().len()'), ('for param in &defs.own_params', 'for param in defs.parameters.iter()'),
        ('param.index as usize', 'param.index() as usize'), ('pub fn ', 'pub(crate) fn '),
    ): constructor = constructor.replace(old, new)
    result['../generic_args.rs'] = fmt(header + """use super::{model::{Args, Arg, Generics, GenericParameter}, context::Context, facts::SourceFacts, identity::DefinitionId};
use rustc_type_ir::{Interner, inherent::*};
// Field projection only: the actual source table is already parent-first.
impl<'cx, F: SourceFacts<'cx>> GenericParameter<'cx, F> {
    pub(crate) fn index(self) -> u32 {
        match self { Self::Type(p) | Self::Const(p, _) => p.index(), Self::Region(p) => p.parameter.index() }
    }
}
impl<'cx, F: SourceFacts<'cx>> Args<'cx, F> {
""" + constructor + '}\n')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        for name, raw in expected().items():
            p = TARGET / name
            if args.write: p.write_bytes(raw)
            if p.read_bytes() != raw: raise ValueError('inference context differs from registered port: ' + name)
        print('Pinned inference context: exact compiler root resolution and selected variable/binder operations; no independent inference algorithm.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n' + getattr(error, 'stderr', b'').decode(errors='replace'))


if __name__ == '__main__': main()

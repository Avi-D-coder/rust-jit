#!/usr/bin/env python3
"""Generate/audit pinned variance inference; only source-context/storage adapters.

The complete constraint walk and fixed point remain upstream algorithms. The
ordinary-source adapter supplies an authenticated closed inference item set.
"""
import argparse
import hashlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_hir_analysis/src/variance'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/variance'
PINS = {
    'mod.rs': '5a9fe3fb38938d03cb7758dac1c338ac818b2807c20e5310e5fae0e00aa06abb',
    'terms.rs': '413cf63d1d9563e20b598f6e2e6d67518eacdb9513e8a250db2d89a988115d25',
    'constraints.rs': 'a2e76421f2acbd0b61276eff8548a180f7fe5bed4b9539be288cdbcf9bb1e0dd',
    'solve.rs': '551dc2a887be981d1d635ea132a5d7b1fed3720f0a75eafb74df71c8c02407ad',
}

def original(name):
    raw = (SOURCE / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != PINS[name]:
        raise ValueError('pinned variance input differs: ' + name)
    return raw.decode()

def replace_function(source, name, replacement):
    marker = 'fn ' + name
    start = source.index(marker)
    # Include a public modifier on the same declaration line.
    start = source.rfind('\n', 0, start) + 1
    opening = source.index('{', source.index(marker))
    depth = 0
    for end in range(opening, len(source)):
        depth += (source[end] == '{') - (source[end] == '}')
        if depth == 0:
            return source[:start] + replacement + source[end + 1:]
    raise ValueError('unterminated pinned function: ' + name)

def common(source):
    for old, new in [
        ("'tcx", "'cx"), ('TyCtxt<\'cx>', "VarianceSource<'a, 'cx>"),
        ('LocalDefIdMap<', 'BTreeMap<DefinitionId, '), ('DefIdMap', 'BTreeMap'),
        ('LocalDefId', 'DefinitionId'), ('DefId', 'DefinitionId'),
        ('ty::CrateVariancesMap<\'cx>', 'VarianceMap'),
        ('ty::CrateVariancesMap', 'VarianceMap'),
        ("Ty<'cx>", "Ty<'cx>"),
        ("GenericArgsRef<'cx>", "Args<'cx>"),
        ("ty::PolyFnSig<'cx>", "ty::Binder<Cx<'cx>, ty::FnSig<Cx<'cx>>>"),
        ("ty::Const<'cx>", "ty::Const<Cx<'cx>>"),
        ("ty::Region<'cx>", "ty::Region<Cx<'cx>>"),
        ("ty::Pattern<'cx>", "<Cx<'cx> as ty::Interner>::Pat"),
        ("ty::TypeAndMut<'cx>", "ty::TypeAndMut<Cx<'cx>>"),
        ('hir::Mutability', 'ty::Mutability'), ('ty::Generics', 'VarianceGenerics'),
        ('ty::GenericParamDefKind', 'VarianceParameterKind'),
        ('*ty.kind()', 'ty.kind()'), ('data.index, variance', 'data.index(), variance'),
        ('def.did()', 'def.definition'), ('def_id.as_local()', 'self.tcx().local_definition(def_id)'),
        ('def_id.to_def_id()', 'def_id'),
        ('span_bug!(\n                    tcx.def_span(def_id),', 'panic!('),
        ('span_bug!(', 'panic!('),
        ('Unexpected unnameable type in variance computation: {ty}',
         'Unexpected unnameable type in variance computation: {ty:?}'),
        ('unexpected type encountered in variance inference: {}',
         'unexpected type encountered in variance inference: {:?}'),
    ]:
        source = source.replace(old, new)
    return re.sub(r'(?<!\w)bug!\(', 'panic!(', source)

HEADER = '''//! Pinned variance inference algorithm; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_variance.py; source-context adapters only.
'''

def expected():
    for name in PINS:
        original(name)
    terms = original('terms.rs')
    terms = terms[terms.index('pub(crate) type VarianceTermPtr'):]
    # Actual demanded closed items replace only HIR crate/constructor discovery.
    terms = replace_function(terms, 'determine_parameters_to_be_inferred', '''pub(crate) fn determine_parameters_to_be_inferred<'a, 'cx>(
    tcx: VarianceSource<'a, 'cx>, arena: &'a Arena<VarianceTerm<'a>>,
) -> TermsContext<'a, 'cx> {
    let mut terms_cx = TermsContext { tcx, arena, inferred_starts: Default::default(),
        inferred_terms: vec![], lang_items: lang_items(tcx) };
    for def_id in tcx.inference_items() {
        terms_cx.add_inferreds_for_item(def_id);
    }
    terms_cx
}''')
    terms = terms.replace("pub arena: &'a DroplessArena", "pub arena: &'a Arena<VarianceTerm<'a>>")
    terms = terms.replace("pub tcx: TyCtxt<'tcx>", "pub tcx: VarianceSource<'a, 'cx>")
    terms = terms.replace("fn lang_items(tcx: TyCtxt<'_>)", "fn lang_items(tcx: VarianceSource<'_, '_>)")
    # Retain the exact three upstream override vectors. The adapter's optional
    # identities describe only the authenticated inference domain.
    terms = terms.replace('let def_id = d?.as_local()?; // LocalDefId', 'let def_id = d?; // authenticated local inference item')
    terms = common(terms)
    terms = HEADER + '''use std::{fmt, collections::BTreeMap};
use typed_arena::Arena;
use rustc_type_ir as ty;
use tracing::debug;
use super::{VarianceSource, DefinitionId};
use self::VarianceTerm::*;
''' + terms

    constraints = original('constraints.rs')
    constraints = constraints[constraints.index('pub(crate) struct ConstraintContext'):]
    # The typed source input already selected the complete demanded item set.
    begin = constraints.index('    let crate_items = tcx.hir_crate_items(());')
    end = constraints.index('\n    constraint_cx\n}', begin)
    constraints = constraints[:begin] + '''    for def_id in tcx.inference_items() {
        constraint_cx.build_constraints_for_item(def_id);
    }
''' + constraints[end:]
    constraints = constraints.replace('let ty = tcx.type_of(def_id).instantiate_identity().skip_norm_wip();', 'let ty = tcx.definition_type(def_id);')
    constraints = constraints.replace('for field in def.all_fields() {', 'for field_ty in tcx.field_types(def_id) {')
    constraints = constraints.replace('tcx.type_of(field.did).instantiate_identity().skip_norm_wip(),', '*field_ty,')
    constraints = constraints.replace('ty::Adt(def, _) => {', 'ty::Adt(_, _) => {')
    constraints = constraints.replace('tcx.fn_sig(def_id).instantiate_identity().skip_norm_wip(),', 'tcx.fn_sig(def_id),')
    constraints = common(constraints)
    constraints = HEADER + '''use rustc_type_ir::{self as ty, GenericArgKind, inherent::*};
use tracing::{debug, instrument};
use super::{VarianceSource, Cx, Ty, Args, DefinitionId};
use super::terms::{VarianceTerm::*, *};
''' + constraints

    solve = original('solve.rs')
    solve = solve[solve.index('fn glb('):]
    solve = solve.replace('FxHashSet::default()', 'HashSet::new()')
    solve = solve.replace('DefIdMap::from(', 'BTreeMap::from_iter(')
    solve = solve.replace('inferred_starts.items()', 'inferred_starts.iter()')
    solve = solve.replace("let variances = tcx.arena.alloc_slice(&solutions[start..(start + count)]);",
                          'let mut variances = solutions[start..(start + count)].to_vec();')
    solve = solve.replace('self.enforce_const_invariance(generics, variances);', 'self.enforce_const_invariance(generics, &mut variances);')
    solve = solve.replace('tcx.type_of(def_id).instantiate_identity().skip_norm_wip().kind()', 'tcx.definition_type(def_id).kind()')
    solve = solve.replace('(def_id.to_def_id(), &*variances)', '(def_id.to_def_id(), variances)')
    solve = solve.replace("-> DefIdMap<&'tcx [ty::Variance]>", '-> VarianceMapEntries')
    solve = common(solve)
    solve = HEADER + '''use std::collections::{BTreeMap, HashSet};
use rustc_type_ir::{self as ty, inherent::*};
use tracing::debug;
use super::{VarianceMap, VarianceMapEntries, VarianceGenerics, VarianceParameterKind};
use super::constraints::*;
use super::terms::{VarianceTerm::*, *};
''' + solve
    result = {}
    for name, text in [('terms.rs', terms), ('constraints.rs', constraints), ('solve.rs', solve)]:
        result[name] = subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(),
            check=True, capture_output=True).stdout
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    for name, data in expected().items():
        path = TARGET / name
        if args.write:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        elif path.read_bytes() != data:
            raise ValueError('variance mechanical port differs: ' + name)
    print('Pinned complete variance terms/constraint walk/fixed point match registered context/storage adaptations.')

if __name__ == '__main__':
    main()

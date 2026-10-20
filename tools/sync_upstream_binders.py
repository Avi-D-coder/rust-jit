#!/usr/bin/env python3
"""Audit/regenerate the pinned compiler's binder client implementations.

The original licensed files stay unchanged. This registers representation and
client-API spelling adaptations only; no new inference or binder algorithm.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
SUPPORT = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty'
FOLD_SHA = 'e07cb2d26e76490d9a5cb6af659d993f5a95e4ccc99088ed90f233e2842f50b4'
PREDICATE_SHA = '181d0e2a00bf8ff81fb367990c6d1e405fc0ab8f8f6be3e35141e48466c7272e'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/binders.rs'


def original(name, digest):
    raw = (SUPPORT / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError('retained compiler binder source differs: ' + name)
    return raw.decode()


def method(source, name):
    """Select a complete named method, including its original comments/body."""
    start = source.index('    pub fn ' + name + '<') if ('    pub fn ' + name + '<') in source else source.index('    pub fn ' + name + '(')
    body = source.index('\n    {', start) + 5 if '\n    {' in source[start:source.index('\n    }', start)] else source.index(' {', start) + 1
    # The pinned selected bodies have balanced braces in their Rust comments.
    depth = 0
    for i in range(body, len(source)):
        if source[i] == '{':
            depth += 1
        elif source[i] == '}':
            depth -= 1
            if depth == 0:
                return source[start:i + 1] + '\n'
    raise ValueError('unterminated pinned method: ' + name)


def adapt(block):
    # Generic ownership and concrete associated-type spelling, not semantics.
    replacements = [
        ("impl<'tcx, D: BoundVarReplacerDelegate<'tcx>>", "impl<'cx, F: SourceFacts<'cx>, D: BoundVarReplacerDelegate<'cx, F>>"),
        ("impl<'tcx, D>", "impl<'cx, F: SourceFacts<'cx>, D>"),
        ("impl<'a, 'tcx>", "impl<'a, 'cx, F: SourceFacts<'cx>>"),
        ("impl<'tcx>", "impl<'cx, F: SourceFacts<'cx>>"),
        ("trait BoundVarReplacerDelegate<'tcx>", "trait BoundVarReplacerDelegate<'cx, F: SourceFacts<'cx>>"),
        ("struct FnMutDelegate<'a, 'tcx>", "struct FnMutDelegate<'a, 'cx, F: SourceFacts<'cx>>"),
        ("struct BoundVarReplacer<'tcx, D>", "struct BoundVarReplacer<'cx, F: SourceFacts<'cx>, D>"),
        ("struct Anonymize<'a, 'tcx>", "struct Anonymize<'a, 'cx, F: SourceFacts<'cx>>"),
        ("BoundVarReplacerDelegate<'tcx>", "BoundVarReplacerDelegate<'cx, F>"),
        ("FnMutDelegate<'a, 'tcx>", "FnMutDelegate<'a, 'cx, F>"),
        ("BoundVarReplacer<'tcx, D>", "BoundVarReplacer<'cx, F, D>"),
        ("Anonymize<'_, 'tcx>", "Anonymize<'_, 'cx, F>"),
        ("TyCtxt<'tcx>", "Context<'cx, F>"),
        ("Binder<'tcx, T>", "Binder<Context<'cx, F>, T>"),
        ("Clause<'tcx>", "Clause<'cx, F>"),
    ]
    for old, new in replacements:
        block = block.replace(old, new)
    block = re.sub(r"\bTy<'tcx>", "Ty<'cx, F>", block)
    block = block.replace("ty::Clauses<'tcx>", "Clauses<'cx, F>")
    for name in ('Region', 'Const', 'BoundRegion', 'BoundTy', 'BoundConst', 'BoundVariableKind', 'Clauses'):
        block = block.replace("ty::" + name + "<'tcx>", 'ty::' + name + "<Context<'cx, F>>")
    block = block.replace('*t.kind()', 't.kind()')
    block = block.replace('pub trait ', 'pub(crate) trait ')
    block = block.replace('pub struct ', 'pub(crate) struct ')
    block = block.replace('pub fn ', 'pub(crate) fn ')
    return block


def expected():
    fold = original('fold.rs', FOLD_SHA)
    predicate = original('predicate.rs', PREDICATE_SHA)
    support = fold[fold.index('/// A delegate used when instantiating bound vars.'):fold.index("impl<'tcx> TyCtxt<'tcx> {")]
    methods = '\n'.join(method(fold, name) for name in (
        'replace_escaping_bound_vars_uncached', 'replace_bound_vars_uncached',
        'shift_bound_var_indices', 'anonymize_bound_vars',
        'instantiate_bound_regions', 'instantiate_bound_regions_uncached', 'instantiate_bound_regions_with_erased',
    ))
    # The concrete source-fact family is named F; rename only the closure parameter.
    for name in ('instantiate_bound_regions','instantiate_bound_regions_uncached'):
        methods=methods.replace(name+'<T, F>',name+'<T, MakeRegion>')
    methods=methods.replace('mut fld_r: F','mut fld_r: MakeRegion').replace('mut replace_regions: F','mut replace_regions: MakeRegion')
    methods=methods.replace('F: FnMut(ty::BoundRegion', 'MakeRegion: FnMut(ty::BoundRegion')
    methods=methods.replace('self.lifetimes.re_erased','self.intern_region(ty::RegionKind::ReErased)').replace('bug!(', 'panic!(')
    binder_impl = "impl<'tcx> TyCtxt<'tcx> {\n" + methods + '}\n'
    supertrait = method(predicate, 'instantiate_supertrait')
    # Match the real shared Interner's predicate-construction API. The compiler
    # client has an extra reuse wrapper; Context::predicate structurally interns
    # the same kind, so the original handle is recovered without a second path.
    supertrait = supertrait.replace("ty::PolyTraitRef<'tcx>", "ty::Binder<Context<'cx, F>, ty::TraitRef<Context<'cx, F>>>")
    supertrait = supertrait.replace('tcx.reuse_or_mk_predicate(\n            self.as_predicate(),', 'tcx.predicate(')
    supertrait = supertrait.replace('.expect_clause()', '.as_clause().expect("upstream substitution constructs a clause")')
    supertrait_impl = "impl<'cx, F: SourceFacts<'cx>> Clause<'cx, F> {\n" + supertrait.replace('pub fn instantiate_supertrait', 'pub fn instantiate_supertrait_upstream') + '}\n'
    header = f'''//! Pinned compiler binder client algorithms; MIT OR Apache-2.0.
//! Pin {PIN}; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_binders.py.
use super::{{context::Context, facts::SourceFacts, model::{{Clause, Clauses, Ty}}}};
use rustc_type_ir::{{self as ty, *, inherent::*, data_structures::{{DelayedMap, IndexMap as FxIndexMap}}}};
use rustc_type_ir::inherent::Ty as _;
'''
    return subprocess.run(
        ['rustfmt', '+1.99.0', '--edition', '2024'],
        input=(header + adapt(support + binder_impl + supertrait_impl)).encode(),
        check=True, capture_output=True,
    ).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        data = expected()
        if args.write:
            TARGET.write_bytes(data)
        if TARGET.read_bytes() != data:
            raise ValueError('LRR binders differ from the registered compiler port')
        print('Pinned binder client: exact originals and registered mechanical port; no solver algorithm changes.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()

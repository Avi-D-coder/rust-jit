#!/usr/bin/env python3
"""Audit/regenerate pinned compiler client relations and const-expression IR.

The retained originals and licenses are Git-owned. These adapters only change
client type/API spelling and compiler-only derives; they add no Rust evaluator.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
SUPPORT = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src'
TARGET = ROOT / 'crates/lrr/src/traits/upstream'
ORIGINALS = {
    'ty/relate.rs': '9bbf1a363657e2c1c6784ada114234054e6502b874a2b0d898573b3c213a17b8',
    'ty/consts/kind.rs': '8d365a93687c9c26ed501ea79ff8760300669cb86b4460ad7405d8fa690ee568',
    'ty/abstract_const.rs': '18f9e57b4a92096a90b12d73a42f0f5c8ed67b83b0a0e5b73b27788c5ab55b44',
    'mir/syntax.rs': 'b2f38c58405cfb5999cbdf50770e151910c796a7f20c82456d37a3f39481f4e1',
    'ty/erase_regions.rs': 'b745df17e5de37c1a4de6577f7dda44dbd07d2002b18a0f32a08907c4553b5b1',
}


def original(name):
    raw = (SUPPORT / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != ORIGINALS[name]:
        raise ValueError('retained compiler relation/IR source differs: ' + name)
    return raw.decode()


def enum(source, name):
    start = source.index('pub enum ' + name + ' {')
    # The pinned metadata enum bodies contain no nested braces.
    return source[start:source.index('\n}', start) + 2]


def common(block):
    replacements = [
        ("impl<'tcx>", "impl<'cx, F: SourceFacts<'cx>>"),
        ("TyCtxt<'tcx>", "Context<'cx, F>"),
        ("ty::GenericArgsRef<'tcx>", "Args<'cx, F>"),
        ("ty::GenericArg<'tcx>", "Arg<'cx, F>"),
        ("RelateResult<'tcx,", "RelateResult<Context<'cx, F>,"),
        ("ty::Pattern<'tcx>", "Pattern<'cx, F>"),
        ("ty::Clauses<'tcx>", "Clauses<'cx, F>"),
        ("ty::Region<'tcx>", "ty::Region<Context<'cx, F>>"),
        ("ty::Const<'tcx>", "ty::Const<Context<'cx, F>>"),
        ("ty::Binder<'tcx, T>", "ty::Binder<Context<'cx, F>, T>"),
        ("ty::Expr<'tcx>", "ExprConst<'cx, F>"),
        ("ty::Expr::", "ExprConst::"),
        ("ty::ExprKind::", "ExprKind::"),
        ('pub fn ', 'pub(crate) fn '),
    ]
    for a, b in replacements:
        block = block.replace(a, b)
    block = re.sub(r"\bTy<'tcx>", "Ty<'cx, F>", block)
    block = re.sub(r"\bConst<'tcx>", "ty::Const<Context<'cx, F>>", block)
    return block


def header(description):
    return f'''//! {description}; MIT OR Apache-2.0.
//! Pin {PIN}; unchanged originals/licenses: vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_relations.py.
'''


def expected():
    mir = original('mir/syntax.rs')
    abstract = original('ty/abstract_const.rs')
    operators = header('Pinned compiler operation metadata, without MIR execution')
    for source, name in [(mir, 'BinOp'), (mir, 'UnOp'), (abstract, 'CastKind')]:
        operators += '#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]\n'
        operators += enum(source, name).replace('pub enum ', 'pub(crate) enum ') + '\n\n'

    source = original('ty/consts/kind.rs')
    begin = source.index('#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]')
    end = source.index('#[cfg(target_pointer_width', begin)
    expression = source[begin:end]
    expression = expression.replace('#[derive(StableHash, TyEncodable, TyDecodable, TypeVisitable, TypeFoldable)]\n', '')
    expression = expression.replace('pub enum ExprKind', 'pub(crate) enum ExprKind')
    expression = expression.replace("pub struct Expr<'tcx>", "pub(crate) struct ExprConst<'cx, F: SourceFacts<'cx>>")
    expression = expression.replace('#[derive(Copy, Clone, Eq, PartialEq, Hash)]', '#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]')
    expression = expression.replace("Expr<'tcx>", "ExprConst<'cx, F>")
    expression = expression.replace('mir::', '')
    expression = expression.replace('self.args().as_slice()', 'self.args().items()')
    expression = re.sub(r'assert_matches!\((.*)\);', r'assert!(matches!(\1));', expression)
    expression = expression.replace('bug!(', 'panic!(')
    expression = header('Pinned compiler const-expression representation and constructors') + '''use super::{context::Context, facts::SourceFacts, model::{Args, Arg, Ty}, expression_operators::{BinOp, UnOp, CastKind}};
use rustc_type_ir::{self as ty, Interner, inherent::*};
''' + common(expression)

    source = original('ty/relate.rs')
    relation = source[source.index("impl<'tcx> Relate<TyCtxt<'tcx>> for ty::Pattern<'tcx>"):source.index("impl<'tcx> Relate<TyCtxt<'tcx>> for ty::GenericArgsRef<'tcx>")]
    relation += source[source.index("impl<'tcx> Relate<TyCtxt<'tcx>> for ty::Expr<'tcx>"):source.index("impl<'tcx> Relate<TyCtxt<'tcx>> for ty::GenericArg<'tcx>")]
    relation = relation.replace("&'tcx ty::List<ty::PolyExistentialPredicate<'tcx>>", "Existentials<'cx, F>")
    relation = relation.replace('.mk_pat(', '.pattern(')
    relation = header('Pinned compiler type-pattern, existential and const-expression relations') + '''use super::{context::Context, facts::SourceFacts, model::{Pattern, Existentials, ExprConst}, constant_expression::ExprKind};
use std::iter;
use rustc_type_ir::{self as ty, inherent::*, relate::{Relate, RelateResult, TypeRelation}, error::{ExpectedFound, TypeError}};
''' + common(relation)
    erase_source = original('ty/erase_regions.rs')
    erasure = erase_source[erase_source.index("fn erase_and_anonymize_regions_ty<'tcx>"):]
    erasure = erasure.replace("fn erase_and_anonymize_regions_ty<'tcx>", "pub(super) fn erase_and_anonymize_regions_ty<'cx, F: SourceFacts<'cx>>")
    erasure = erasure.replace("struct RegionEraserAndAnonymizerVisitor<'tcx>", "struct RegionEraserAndAnonymizerVisitor<'cx, F: SourceFacts<'cx>>")
    erasure = erasure.replace("RegionEraserAndAnonymizerVisitor<'tcx>", "RegionEraserAndAnonymizerVisitor<'cx, F>")
    erasure = erasure.replace('self.tcx.lifetimes.re_erased', 'self.tcx.intern_region(ty::ReErased)')
    expansion = abstract[abstract.index("impl<'tcx> TyCtxt<'tcx> {"):]
    expansion = expansion.replace("struct Expander<'tcx>", "struct Expander<'cx, F: SourceFacts<'cx>>")
    expansion = expansion.replace("Expander<'tcx>", "Expander<'cx, F>")
    expansion = expansion.replace('self.tcx.thir_abstract_const(def_id)', 'self.tcx.services().abstract_const(self.tcx, def_id)')
    constants = header('Pinned compiler abstract-constant expansion and region erasure') + '''use super::{context::Context, facts::SourceFacts, model::{Ty, Clauses}};
use rustc_type_ir::{self as ty, *, inherent::*};
use tracing::debug;
''' + common(erasure + expansion)
    outputs = {}
    for name, text in [('expression_operators.rs', operators), ('constant_expression.rs', expression), ('relations.rs', relation), ('abstract_consts.rs', constants)]:
        outputs[name] = subprocess.run(
            ['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(),
            check=True, capture_output=True,
        ).stdout
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        for name, raw in expected().items():
            path = TARGET / name
            if args.write:
                path.write_bytes(raw)
            if path.read_bytes() != raw:
                raise ValueError('LRR client differs from registered compiler port: ' + name)
        print('Pinned relation/constant IR: exact originals, registered mechanical ports; no evaluator or solver changes.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()

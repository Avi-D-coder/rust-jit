#!/usr/bin/env python3
"""Regenerate/audit pinned rustc value-tree helpers used by the WF visitor.

Only model/lifetime/field spelling, borrowed-slice representation, visibility and
compiler bug diagnostic transport differ. No constant evaluation is performed.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess

from sync_upstream_binders import method

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
PINS = {
    'compiler/rustc_middle/src/ty/consts/valtree.rs': '1ab0c5918c3a8b8ff86a3eef72bb726e559ba43d9503433b7ab2c153d7368b81',
    'compiler/rustc_middle/src/ty/consts.rs': '6337715239e9bf252c749c85d1c19fb2f4b4c5fad2430046f1bcc106ceb2c52c',
    'compiler/rustc_middle/src/ty/consts/int.rs': 'adc87f2118e4023afcd21b22dfc94553471eca2458d73515336d02808c1da1b9',
    'compiler/rustc_middle/src/ty/mod.rs': 'bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7',
}
TARGET = ROOT / 'crates/lrr/src/traits/upstream/wf_values.rs'


def original(name):
    data = (ROOT / 'vendor/rustc-trait-solver' / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != PINS[name]:
        raise ValueError('pinned value helper source differs: ' + name)
    return data.decode()


def picked(source, name):
    # The compiler ConstExt/check_data helpers are private trait/impl methods.
    return method(source.replace('\n    fn ', '\n    pub fn '), name)


def adapt_types(text):
    return (text.replace("'tcx", "'cx")
            .replace("ty::Const<'cx>", "ty::Const<Context<'cx, F>>")
            .replace("ty::Value<'cx>", "ValueConst<'cx, F>")
            .replace("ty::DestructuredAdtConst<'cx>", "DestructuredAdtConst<'cx, F>")
            .replace('ty::DestructuredAdtConst {', 'DestructuredAdtConst {')
            .replace('ty::Adt(', 'ty::TyKind::Adt(')
            .replace('bug!(', 'panic!('))


def expected():
    tree = original('compiler/rustc_middle/src/ty/consts/valtree.rs')
    constants = original('compiler/rustc_middle/src/ty/consts.rs')
    scalar = original('compiler/rustc_middle/src/ty/consts/int.rs')
    types = original('compiler/rustc_middle/src/ty/mod.rs')

    # Const is the shared crate's foreign type; use the compiler extension-trait
    # pattern, keeping to_value/to_leaf as the identical composed operations.
    const_methods = ''.join(picked(constants, name) for name in ('to_value', 'to_leaf'))
    const_methods = adapt_types(const_methods).replace('pub fn ', 'fn ')
    const_impl = "impl<'cx, F: SourceFacts<'cx>> ConstValueExt<'cx, F> for ty::Const<Context<'cx, F>> {\n" + const_methods + '}\n'

    value_methods = ''.join(picked(tree, name) for name in ('to_leaf', 'to_branch', 'destructure_adt_const'))
    value_methods = adapt_types(value_methods).replace('pub fn ', 'pub(crate) fn ')
    # Model ValTree and List expose kind()/items() instead of compiler Deref.
    value_methods = value_methods.replace('match &**self.valtree {', 'match self.tree.kind() {')
    value_methods = value_methods.replace('ValTreeKind::Leaf(s) => *s,', 'ValTreeKind::Leaf(s) => s,')
    value_methods = value_methods.replace('ValTreeKind::Branch(branch) => &**branch,', 'ValTreeKind::Branch(branch) => branch.items(),')
    value_impl = "impl<'cx, F: SourceFacts<'cx>> ValueConst<'cx, F> {\n" + value_methods + '}\n'

    # Keep every transitive scalar size/data check from the pinned compiler.
    # The existing model uses bytes:u8 and bits:u128 for size:NonZero<u8>/data.
    scalar_methods = ''.join(picked(scalar, name) for name in ('size', 'check_data', 'to_bits', 'to_uint', 'to_u32'))
    scalar_methods = scalar_methods.replace('self.size.get()', 'self.bytes')
    scalar_methods = scalar_methods.replace('self.data', 'self.bits')
    scalar_methods = scalar_methods.replace('self.size\n', 'self.bytes\n')
    scalar_methods = scalar_methods.replace('pub fn ', 'pub(crate) fn ')
    scalar_impl = 'impl ScalarInt {\n' + scalar_methods + '}\n'

    start = types.index("pub struct DestructuredAdtConst<'tcx>")
    end = types.index('\n}', start) + 2
    destructured = adapt_types(types[start:end])
    destructured = destructured.replace("pub struct DestructuredAdtConst<'cx>", "pub(crate) struct DestructuredAdtConst<'cx, F: SourceFacts<'cx>>")
    text = PRELUDE + destructured + '\n' + const_impl + value_impl + scalar_impl
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(), capture_output=True, check=True).stdout


PRELUDE = '''//! Pinned rustc value-tree helpers; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_wf_values.py.
use super::{context::Context, facts::SourceFacts, model::{ScalarInt, ValueConst}};
use rustc_abi::{Size, VariantIdx, FIRST_VARIANT};
use rustc_type_ir::{self as ty, inherent::IntoKind, ValTreeKind};

/// Pinned compiler ConstExt operations for the shared foreign Const type.
pub(crate) trait ConstValueExt<'cx, F: SourceFacts<'cx>> {
    fn to_value(self) -> ValueConst<'cx, F>;
    fn to_leaf(self) -> ScalarInt;
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
        raise ValueError('WF value helpers differ from registered pinned transformation')
    print('Pinned WF value helper source hashes and registered transformations match; build/root/runtime acceptance is separate.')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Audit/regenerate the pinned compiler's exhaustive shared-IR Ty traversal.

No runtime Rust parser or type evaluator: this mechanically ports one retained
compiler support block. --write edits only the named LRR adapter source.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty/structural_impls.rs'
SOURCE_SHA256 = '2bac6a97bc8670297081a013cc744b22dbf93cf3455a60e1596ef1d9d5e279db'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/traversal.rs'


def expected():
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA256:
        raise ValueError('retained compiler traversal source differs from its pin')
    source = raw.decode()
    start = source.index("impl<'tcx> TypeSuperFoldable<TyCtxt<'tcx>> for Ty<'tcx> {")
    end = source.index("\nimpl<'tcx> TypeFoldable<TyCtxt<'tcx>> for ty::Predicate<'tcx>", start)
    block = source[start:end]
    block = block.replace("impl<'tcx>", "impl<'cx, S:SourceFacts<'cx>>")
    block = block.replace("TyCtxt<'tcx>", "Context<'cx,S>")
    block = block.replace("Ty<'tcx>", "Ty<'cx,S>")
    block = block.replace('*self.kind()', 'self.kind()')
    block = block.replace('.mk_ty_from_kind(kind)', '.ty(kind)')
    header = f'''//! Exhaustive Ty traversal adapted mechanically from pinned rustc_middle.
//! MIT OR Apache-2.0; retained original and licenses: vendor/rustc-trait-solver.
//! Pin {PIN}, ty/structural_impls.rs.
use super::{{context::Context,facts::SourceFacts,model::Ty}};
use rustc_type_ir::{{self as ty,*,inherent::IntoKind}};
'''
    return subprocess.run(
        ['rustfmt', '+1.99.0', '--edition', '2024'],
        input=(header + block).encode(), check=True, capture_output=True,
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
            raise ValueError('LRR traversal differs from the registered mechanical compiler port')
        print('Pinned Ty traversal: exact source hash, registered mechanical port, Rust 1.99.0 formatting; no semantic changes.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Audit the pinned compiler's relevant-impl index traversal client port."""
import argparse
import hashlib
from pathlib import Path
import subprocess
from sync_upstream_binders import method

ROOT = Path(__file__).resolve().parents[1]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
ORIGINAL = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty/trait_def.rs'
SHA = 'a4c51634eee9ad9a5944256bfbcf8f4d6ac09c034ba6f34acd844eb30646608d'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/candidates.rs'


def expected():
    raw = ORIGINAL.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise ValueError('retained relevant-impl client source differs from its pin')
    block = method(raw.decode(), 'for_each_relevant_impl')
    block = block.replace('pub fn for_each_relevant_impl', 'pub(super) fn for_each_relevant_source_impl')
    block = block.replace('        self,', "        self, cx: Cx<'cx>,", 1)
    block = block.replace('trait_def_id: DefId', 'trait_def_id: DefinitionId')
    block = block.replace('FnMut(DefId)', 'FnMut(DefinitionId)')
    block = block.replace('ty::SimplifiedType', 'ty::fast_reject::SimplifiedType')
    block = block.replace("Ty<'tcx>", "Ty<'cx, PreparedProvider<'cx>>")
    block = block.replace('let tcx = self;', 'let tcx = cx;')
    block = block.replace('tcx.trait_impls_of(trait_def_id)', 'self.prepared_impls(trait_def_id)')
    block = block.replace('#[allow(rustc::usage_of_type_ir_traits)]', '')
    block = block.replace('self.for_each_blanket_impl(trait_def_id, f)', 'SourceFacts::for_each_blanket_impl(self, cx, trait_def_id, f)')
    block = block.replace('bug!(', 'panic!(')
    text = f'''//! Pinned compiler relevant-impl index traversal; MIT OR Apache-2.0.
//! Pin {PIN}; original/licenses in vendor/rustc-trait-solver.
//! Mechanical adapter: tools/sync_upstream_candidates.py; no solver matching here.
use super::{{prepared::{{Cx, PreparedProvider}}, identity::DefinitionId, facts::SourceFacts, model::Ty}};
use rustc_type_ir::{{self as ty, VisitorResult, try_visit, inherent::IntoKind}};
impl<'cx> PreparedProvider<'cx> {{
''' + block + '}\n'
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=text.encode(), check=True, capture_output=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        data = expected()
        if args.write:
            TARGET.write_bytes(data)
        if TARGET.read_bytes() != data:
            raise ValueError('relevant-impl client differs from registered compiler port')
        print('Pinned relevant-impl client: exact original and registered mechanical adapter; no custom matching.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()

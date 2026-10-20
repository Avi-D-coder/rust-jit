#!/usr/bin/env python3
"""Generate/audit complete pinned GenericClauses instantiation and storage.

Only context/types/lifetimes and the immutable typed parent-query adapter differ.
No reflection policy, source collector, predicate closure or normalization lives
in this port.
"""
import argparse
import hashlib
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/generic_clauses.rs'
PINS = {
    'generics.rs': 'f5231c1921d2691ee02305a0c3987e420487c6b841be256c10eafe2752b935b0',
    'mod.rs': 'bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7',
}

def original(name):
    raw = (SOURCE / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != PINS[name]:
        raise ValueError('pinned generic-clause source differs: ' + name)
    return raw.decode()

def spellings(text):
    for old, new in [
        ("'tcx", "'cx"), ("TyCtxt<'cx>", "Cx<'cx>"),
        ("GenericArgsRef<'cx>", "Args<'cx>"),
        ("Unnormalized<'cx, Clause<'cx>>", "Unnormalized<Cx<'cx>, Clause<'cx>>"),
        ('pub fn ', 'pub(super) fn '), ('pub struct ', 'pub(super) struct '),
    ]:
        text = text.replace(old, new)
    return text

SCAFFOLD = '''//! Pinned GenericClauses instantiation; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_generic_clauses.py.
use super::{identity::DefinitionId, model, prepared::{Cx, PreparedProvider}};
use rustc_type_ir::{EarlyBinder, Unnormalized};
use std::marker::PhantomData;
use tracing::instrument;
type Clause<'cx> = model::Clause<'cx, PreparedProvider<'cx>>;
type Args<'cx> = model::Args<'cx, PreparedProvider<'cx>>;
type Span = model::Span;

/// Query identity is a type boundary. An explicit-source bundle cannot stand
/// in for compiler full predicates merely because their storage shapes match.
pub(super) trait ClauseQuery: Copy {}
#[derive(Clone, Copy, Debug)]
pub(super) struct ExplicitOwnClauses;
impl ClauseQuery for ExplicitOwnClauses {}
#[derive(Clone, Copy, Debug)]
pub(super) struct FullClauses;
impl ClauseQuery for FullClauses {}

/// The actual source bundle owns the authenticated parent receipts and encoded
/// clauses. This callback reads that immutable retained map, never live source
/// queries or a partially populated SourceFacts provider.
pub(super) trait ClauseParents<'data, 'cx>: Copy {
    type Query: ClauseQuery;
    fn clauses_of(self, definition: DefinitionId) -> GenericClauses<'data, 'cx, Self::Query>;
}

/// Same compiler query payload with a separate borrow lifetime for an owned
/// source bundle. No Default: absence must be established by its real producer.
#[derive(Clone, Copy, Debug)]
pub(super) struct GenericClauses<'data, 'cx, Q: ClauseQuery> {
    parent: Option<DefinitionId>,
    clauses: &'data [(Clause<'cx>, Span)],
    query: PhantomData<Q>,
}
impl<'data, 'cx, Q: ClauseQuery> GenericClauses<'data, 'cx, Q> {
    /// Only the actual prepared source-bundle factory supplies this payload.
    /// It authenticates own clauses, query kind and the exact source parent.
    pub(super) fn from_source(parent: Option<DefinitionId>, clauses: &'data [(Clause<'cx>, Span)]) -> Self {
        Self { parent, clauses, query: PhantomData }
    }
}
'''

def expected():
    generics = original('generics.rs')
    methods = generics.split("impl<'tcx> GenericClauses<'tcx> {", 1)[1]
    methods = methods.split('    /// Allow simple where bounds', 1)[0]
    methods = spellings(methods)
    parent_bound = "<P: ClauseParents<'data, 'cx, Query = Q>>"
    for name in ['instantiate', 'instantiate_into', 'instantiate_identity', 'instantiate_identity_into']:
        methods = methods.replace('fn ' + name + '(', 'fn ' + name + parent_bound + '(')
    methods = methods.replace("        args: Args<'cx>,\n    ) -> InstantiatedClauses<'cx>",
                              "        args: Args<'cx>,\n        parents: P,\n    ) -> InstantiatedClauses<'cx>")
    methods = methods.replace("        args: Args<'cx>,\n    ) {", "        args: Args<'cx>,\n        parents: P,\n    ) {")
    methods = methods.replace("fn instantiate_identity" + parent_bound + "(self, tcx: Cx<'cx>)",
                              "fn instantiate_identity" + parent_bound + "(self, tcx: Cx<'cx>, parents: P)")
    methods = methods.replace("        instantiated: &mut InstantiatedClauses<'cx>,\n    ) {",
                              "        instantiated: &mut InstantiatedClauses<'cx>,\n        parents: P,\n    ) {")
    methods = methods.replace('skip(self, tcx)', 'skip(self, tcx, parents)')
    methods = methods.replace('self.instantiate_into(tcx, &mut instantiated, args)',
                              'self.instantiate_into(tcx, &mut instantiated, args, parents)')
    methods = methods.replace('self.instantiate_identity_into(tcx, &mut instantiated)',
                              'self.instantiate_identity_into(tcx, &mut instantiated, parents)')
    methods = methods.replace('tcx.clauses_of(def_id).instantiate_into(tcx, instantiated, args)',
                              'parents.clauses_of(def_id).instantiate_into(tcx, instantiated, args, parents)')
    methods = methods.replace('tcx.clauses_of(def_id).instantiate_identity_into(tcx, instantiated)',
                              'parents.clauses_of(def_id).instantiate_identity_into(tcx, instantiated, parents)')
    methods = methods.replace('iter_instantiated_copied(tcx, args)', 'iter_instantiated_copied(tcx, args.items())')
    # Span is compiler metadata, already normalized. The upstream wrapper is
    # preserved and the local Span implements the same trivial fold contract.
    middle = original('mod.rs')
    start = middle.index('#[derive(Clone, Debug)]\npub struct InstantiatedClauses')
    end = middle.index('\n#[derive(', start+1)
    storage = spellings(middle[start:end])
    text = SCAFFOLD + "\nimpl<'data, 'cx, Q: ClauseQuery> GenericClauses<'data, 'cx, Q> {" + methods + '\n}\n' + storage
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'],
                          input=text.encode(), check=True, capture_output=True).stdout

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    data = expected()
    if args.write:
        TARGET.write_bytes(data)
    elif TARGET.read_bytes() != data:
        raise ValueError('generic-clause mechanical port differs')
    print('Pinned complete generic-clause instantiation/storage match registered context/lifetime/parent-query adaptations.')

if __name__ == '__main__':
    main()

"""Pinned mechanical stable build projection for the compiler relation helper.

No relation algorithm is implemented here. Exact originals remain Git inputs;
all changes below concern module/import paths or replacing stable panic stubs
with the same helper used by the pinned compiler's nightly build.
"""
import hashlib

ADAPTATION = 'stable-region-assumptions-v2-leak-indexes'
REGION = 'compiler/rustc_type_ir/src/region_constraint.rs'
DATA = 'compiler/rustc_type_ir/src/data_structures/mod.rs'
PREFIX = 'compiler/rustc_data_structures/src/'
LEAK = 'compiler/rustc_infer/src/infer/region_constraints/leak_check.rs'
INDEXES = DATA.removesuffix('mod.rs') + 'leak_check_indexes.rs'
SUPPORT_SOURCES = tuple(PREFIX + name for name in (
    'frozen.rs', 'transitive_relation.rs', 'transitive_relation/tests.rs')) + (LEAK,)
HASHES = {
    REGION: '444abc494b01d457e246f59d8430c72ce088ded95c81cc60e48af5974ee76a91',
    DATA: '60d0030b0a4f7d4ff7440d0ad608eee0e9212469829e1aab8c5aa613686cd690',
    SUPPORT_SOURCES[0]: '09a4a7d611ad449efbdb89bf30da347050c363bbbcf198fc80784df19c65e3c1',
    SUPPORT_SOURCES[1]: 'f549d6c82b78d3e9fe2e838040dc8941fef6e70f0fa2ba09a23cb8e63803ff4f',
    SUPPORT_SOURCES[2]: 'a0b9f92d4c889a033a915cbd6d83b5b8a6e87555a3bcd19a43db032c49dabc74',
    LEAK: '14eb7aeafc432c5058d7176b2d34889879c84584d568311d773cb13a93683d42',
}
RETAIN_ORIGINAL = (REGION, DATA)
PROJECTION_SOURCES = {REGION: REGION, DATA: DATA, INDEXES: LEAK, **{
    DATA.removesuffix('mod.rs') + path.removeprefix(PREFIX): path
    for path in SUPPORT_SOURCES[:3]
}}


def index_declarations(source):
    return source[source.index('rustc_index::newtype_index! {'):
                  source.index('/// Represents the graph of constraints.')]


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError('pinned stable adaptation anchor differs: ' + old)
    return source.replace(old, new, 1)


def projections(originals):
    for path, expected in HASHES.items():
        if hashlib.sha256(originals[path]).hexdigest() != expected:
            raise ValueError('stable helper original differs: ' + path)
    region = originals[REGION].decode()
    start = region.index('// Workaround for TransitiveRelation being')
    stop = region.index('use crate::data_structures::IndexMap;', start)
    region = region[:start] + '''// Stable build uses the exact pinned compiler helper; see retained .upstream.
#[cfg(not(feature = "nightly"))]
pub use crate::data_structures::{TransitiveRelation, TransitiveRelationBuilder};

''' + region[stop:]
    data = originals[DATA].decode() + '''
// Stable build of the exact pinned rustc_data_structures helper.
#[cfg(not(feature = "nightly"))]
mod frozen;
#[cfg(not(feature = "nightly"))]
pub use frozen::Frozen;
#[cfg(not(feature = "nightly"))]
mod transitive_relation;
#[cfg(not(feature = "nightly"))]
pub use transitive_relation::{TransitiveRelation, TransitiveRelationBuilder};
pub mod leak_check_indexes;
'''
    frozen = replace_once(originals[SUPPORT_SOURCES[0]].decode(),
                          'use rustc_data_structures::frozen::Frozen;',
                          'use rustc_type_ir::data_structures::Frozen;')
    relation = replace_once(originals[SUPPORT_SOURCES[1]].decode(),
                            'use crate::frozen::Frozen;',
                            'use crate::data_structures::Frozen;')
    relation = replace_once(relation, 'use crate::fx::{FxHashSet, FxIndexSet};',
                             'use crate::data_structures::{HashSet as FxHashSet, IndexSet as FxIndexSet};')
    base = DATA.removesuffix('mod.rs')
    indexes = index_declarations(originals[LEAK].decode())
    indexes = indexes.replace('    #[orderable]', '    #[gate_rustc_only]\n    #[orderable]')
    indexes = indexes.replace('    struct LeakCheck', '    pub struct LeakCheck')
    return {REGION: region.encode(), DATA: data.encode(),
            base + 'frozen.rs': frozen.encode(),
            base + 'transitive_relation.rs': relation.encode(),
            base + 'transitive_relation/tests.rs': originals[SUPPORT_SOURCES[2]],
            INDEXES: ('// Exact pinned compiler index declarations; original retained in infer/region_constraints/leak_check.rs.\n' + indexes.rstrip() + '\n').encode()}

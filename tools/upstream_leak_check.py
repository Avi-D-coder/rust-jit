"""Pinned compiler graph/SCC/test closure and actual region leak checker port.

No local graph, universe, propagation or leak-check algorithm is supplied.
"""
import hashlib
from pathlib import Path
from upstream_region_collector import adapt, selected
from upstream_solver_stable import index_declarations

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'vendor/rustc-trait-solver'
GRAPH = 'compiler/rustc_data_structures/src/graph/'
LEAK = 'compiler/rustc_infer/src/infer/region_constraints/leak_check.rs'
HASHES = {
    GRAPH+'mod.rs': '13eacc316d822069c30f3364cde0852c14e40a14d7c5447735def594ecb6da4b',
    GRAPH+'reference.rs': 'b10fe50c260e472c0d8cea7b4af12dd367c2040c28f8dee6ad56ab6a5357d6b3',
    GRAPH+'reversed.rs': '867228a6a49bb0e922b0f25157d3cd9d70a43c58d34685a361750c30e9de2b99',
    GRAPH+'tests.rs': '86b19cdbfb5a9a8212e231ca24561a07d216e40705744beaa405393d7fdc74dd',
    GRAPH+'iterate/mod.rs': '5515172855a4ced57f1b00b0db2aa59f7f2c5561c5799cacea3bce8b7470e971',
    GRAPH+'iterate/tests.rs': 'bab78a225bdfe2a56ea270c7328979a31ca4dbb5082b1e3d17d9f6e795536bde',
    GRAPH+'scc/mod.rs': '1c4ec37560e3f657cb32781855d630939ffe3cf11b63dd80cd9c5f2dce3565b9',
    GRAPH+'scc/tests.rs': '8635a8400b34013d67f7c4636d4ff6dbb4c533a659ee6141aec90b04d8a3004e',
    GRAPH+'vec_graph/mod.rs': '5f4b8ec94e6cfccb59edb90650639afdc24744010ad52e2efcc148cf1377ef9d',
    GRAPH+'vec_graph/tests.rs': '8483b13073eab355c556c5cbe44196de8ca73a13bb9f2111f2a328d4d704106d',
    LEAK: '14eb7aeafc432c5058d7176b2d34889879c84584d568311d773cb13a93683d42',
}


def original(path):
    raw = (SUPPORT/path).read_bytes()
    if hashlib.sha256(raw).hexdigest() != HASHES[path]:
        raise ValueError('compiler graph/checker original differs: ' + path)
    return raw.decode()


def extend_leak_check(outputs, header, rustfmt):
    for path in HASHES:
        if not path.startswith(GRAPH):continue
        name = path.removeprefix(GRAPH)
        text = original(path).replace('crate::graph', 'crate::traits::upstream::graph')
        text = text.replace('crate::fx::', 'rustc_hash::')
        if name == 'mod.rs':
            text = text.replace('pub mod dominators;\n', '').replace('pub mod linked_graph;\n', '')
        if name == 'scc/mod.rs':
            text = text.replace('use std::debug_assert_matches;\n', '')
            text = text.replace('debug_assert_matches!(self.node_states[node], NodeState::NotVisited);',
                                'debug_assert!(matches!(self.node_states[node], NodeState::NotVisited));')
        if name == 'scc/tests.rs':
            # Stable test packaging only. All23 #[test] bodies are retained;
            # the original benchmark remains in the file, cfg-excluded explicitly.
            text = text.replace('extern crate test;', '#[cfg(any())]\nextern crate test;')
            text = text.replace('#[bench]', '#[cfg(any())]\n#[bench]')
        if name == 'vec_graph/tests.rs':
            # LRR also links serde_json's PartialEq<usize> implementation; make
            # the existing empty expected slices unambiguous without changing them.
            if text.count(', &[]);') != 3:
                raise ValueError('graph empty slice annotation anchors differ')
            text = text.replace(', &[]);', ', &[] as &[usize]);')
        outputs['../graph/'+name] = rustfmt(header + text)
    text = original(LEAK)
    text = text.replace(index_declarations(text),
        'use rustc_type_ir::data_structures::leak_check_indexes::{LeakCheckNode, LeakCheckScc};\n\n')
    text = text.replace('use rustc_data_structures::fx::FxIndexMap;', 'use super::super::FxIndexMap;')
    text = text.replace('rustc_data_structures::graph', 'crate::traits::upstream::graph')
    text = text.replace('use rustc_middle::ty::error::TypeError;\n', '')
    text = text.replace('use rustc_span::span_bug;\n', '')
    text = text.replace('use crate::infer::relate::RelateResult;', '''type RelateResult<'db, T> = rustc_type_ir::relate::RelateResult<DbInterner<'db>, T>;
type TypeError<'db> = rustc_type_ir::error::TypeError<DbInterner<'db>>;''')
    text = text.replace('use crate::infer::snapshot::CombinedSnapshot;', 'use super::super::snapshot::CombinedSnapshot;')
    text = adapt(text).replace('self.interner', 'self.tcx')
    text = text.replace("ty::PlaceholderRegion<'db>", "ty::PlaceholderRegion<DbInterner<'db>>")
    text = text.replace("RegionConstraintCollector<'_, 'db>", "RegionConstraintCollector<'db, '_>")
    text = text.replace("RegionConstraintCollector<'a, 'db>", "RegionConstraintCollector<'db, 'a>")
    text = text.replace("CombinedSnapshot<'db>", 'CombinedSnapshot')
    text = text.replace('ty::RePlaceholder', 'ty::RegionKind::RePlaceholder')
    text = text.replace('''&AddVerify(i) => span_bug!(
                        region_constraints.data().verifys[i].origin.span(),
                        "we never add verifications while doing higher-ranked things",
                    ),''', '''&AddVerify(i) => panic!(
                        "we never add verifications while doing higher-ranked things at {:?}",
                        region_constraints.data().verifys[i].origin,
                    ),''')
    outputs['region_constraints/leak_check.rs'] = rustfmt(header + text)
    # Actual Verify origin and insertion methods, used by the invariant branch.
    compiler = (SUPPORT/'compiler/rustc_infer/src/infer/region_constraints/mod.rs').read_text()
    if hashlib.sha256(compiler.encode()).hexdigest() != '01e53909ea1a62b3415889611c7593e3d226c3a1eadf19f6dee4525b368fd5cb':
        raise ValueError('compiler region collector original differs')
    marker = '#[derive(Debug, Clone)]\npub struct Verify'
    start = compiler.index(marker);stop = compiler.index('\n}', start)+2
    body = outputs['region_constraints/mod.rs'].decode()
    first = body.index(marker);last = body.index('\n}', first)+2
    body = body[:first] + adapt(compiler[start:stop]) + body[last:]
    body = body.replace('use std::ops::Range;', 'mod leak_check;\nuse std::ops::Range;', 1)
    body = body.replace('    #[expect(dead_code, reason = "this is used in rustc")]\n', '')
    methods = selected(compiler, 'add_verify').replace('    pub fn ', '    fn ', 1)
    methods += selected(compiler, 'verify_generic_bound').replace('    pub fn ', '    pub(super) fn ', 1)
    body += "\nimpl<'db, 'a> RegionConstraintCollector<'db, 'a> {\n" + adapt(methods) + '}\n'
    outputs['region_constraints/mod.rs'] = rustfmt(body)

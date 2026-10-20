#!/usr/bin/env python3
"""Audit the pinned nominal type_of payload and the real-source admission wrapper."""
import hashlib
import re
from pathlib import Path
from sync_upstream_binders import method
ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_hir_analysis/src/collect/type_of.rs'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/source_definition_headers.rs'
SHA = 'c643bb4fcd51048f7d3e912065c2759d4a91e51b5a2e726bdc30a998a4ee3730'

def normalize(text):
    return re.sub(r'\s+', '', re.sub(r'//[^\n]*', '', text))

def main():
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise ValueError('pinned nominal type_of source differs')
    source = raw.decode()
    match = re.search(r'ItemKind::Enum\(\.\.\)\s*\|\s*ItemKind::Struct\(\.\.\)\s*\|\s*ItemKind::Union\(\.\.\)\s*=>\s*\{([^{}]*)\}', source)
    if match is None:
        raise ValueError('pinned nominal type_of branch absent')
    payload = match.group(1).replace('tcx', 'cx').replace('def_id', 'self.definition')
    payload = payload.replace('ty::GenericArgs', 'super::model::Args')
    payload = payload.replace('Ty::new_adt(cx, def, args)',
        'Ok(rustc_type_ir::EarlyBinder::bind(cx, super::model::Ty::new_adt(cx, def, args)))')
    # The original provider wraps its actual output in the same early binder.
    if 'ty::EarlyBinder::bind(tcx, output)' not in source:
        raise ValueError('pinned type_of output binder differs')
    expected = '''pub fn nominal_type(&self, cx: Cx<'cx>)
        -> Result<rustc_type_ir::EarlyBinder<Cx<'cx>, super::model::Ty<'cx, prepared::PreparedProvider<'cx>>>, DefinitionTypePending>
    {
        use rustc_type_ir::inherent::*;
        if !matches!(self.role, DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union) {
            return Err(DefinitionTypePending::NonNominal);
        }
        if self.definition.world() != cx.services().source_world() { return Err(DefinitionTypePending::SourceWorld); }
        if cx.services().require_generic_value(self.definition, self.generics).is_err() { return Err(DefinitionTypePending::SourceGenerics); }
    ''' + payload + '\n    }\n'
    actual = method(TARGET.read_text().replace('pub(super) fn ', 'pub fn '), 'nominal_type')
    if normalize(expected) != normalize(actual):
        raise ValueError('nominal type_of payload or source admission wrapper differs')
    print('Pinned nominal type_of payload and actual-source guards match; other kinds/factory/root execution remain held.')

if __name__ == '__main__':
    main()

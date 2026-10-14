#!/usr/bin/env python3
"""Exercise the actual pinned patch proof, including rejected preimages and receipts."""
import copy
import json
import tarfile
import tomllib
import unittest
import bootstrap_vendor as vendor


class PatchProof(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.provenance = tomllib.loads((vendor.ROOT / 'vendor/UPSTREAM.toml').read_text())['rust_analyzer_macros']
        cls.inventory = json.loads(vendor.INVENTORY.read_text())
        cls.record = cls.provenance['patches'][0]
        cls.patch = (vendor.ROOT / 'vendor' / cls.record['patch']).read_bytes()
        with tarfile.open(vendor.ARCHIVE, 'r:gz') as archive:
            cls.original = archive.extractfile(f"rust-analyzer-{cls.provenance['commit']}/{cls.record['path']}").read()

    def test_real_patch_reconstructs_current_source(self):
        receipts = vendor.audit()['patches']
        with tarfile.open(vendor.ARCHIVE, 'r:gz') as archive:
            for index, record in enumerate(self.provenance['patches']):
                original = archive.extractfile(f"rust-analyzer-{self.provenance['commit']}/{record['path']}").read()
                patch = (vendor.ROOT / 'vendor' / record['patch']).read_bytes()
                patched = vendor.apply_text_patch(original, patch, record['path'])
                self.assertEqual(patched, (vendor.TREE / record['path']).read_bytes())
                self.assertEqual(vendor.sha(patched), record['postimage_sha256'])
                self.assertEqual(receipts[index]['postimage_sha256'], vendor.sha(patched))

    def test_changed_context_and_wrong_target_refuse_patch(self):
        changed = self.original.replace(b'Binding::Nested(bs) => bs.get', b'Binding::Nested(bs) => bs.bad', 1)
        self.assertNotEqual(changed, self.original)
        with self.assertRaisesRegex(ValueError, 'preimage/context mismatch'):
            vendor.apply_text_patch(changed, self.patch, self.record['path'])
        with self.assertRaisesRegex(ValueError, 'target header mismatch'):
            vendor.apply_text_patch(self.original, self.patch, 'crates/other.rs')

    def test_forged_receipt_hashes_refuse_original_patch(self):
        for index in range(len(self.provenance['patches'])):
            for field, message in [('sha256', 'patch hash'), ('preimage_sha256', 'preimage differs'), ('postimage_sha256', 'postimage hash')]:
                forged = copy.deepcopy(self.provenance)
                forged['patches'][index][field] = '0' * 64
                with self.assertRaisesRegex(ValueError, message):
                    vendor.patched_inventory(self.inventory, forged, self.provenance['commit'])

    def test_hygiene_entrypoint_reuses_entire_archived_mark_algorithm(self):
        target = 'crates/hir-expand/src/hygiene.rs'
        with tarfile.open(vendor.ARCHIVE, 'r:gz') as archive:
            original = archive.extractfile(f"rust-analyzer-{self.provenance['commit']}/{target}").read().decode()
        current = (vendor.TREE / target).read_text()
        marker = '    let mut call_site_ctxt = if transparency == Transparency::SemiOpaque {'
        suffix = original[original.index(marker):].replace('db: &dyn SourceDatabase', 'db: &dyn salsa::Database')
        self.assertEqual(suffix, current[current.index(marker):])
        self.assertIn('apply_mark_with_call_site(db, ctxt, call_id, transparency, edition, call_site_ctxt)', current)

    def test_concat_entrypoint_preserves_archived_body_with_audited_numeric_repair(self):
        target = 'crates/hir-expand/src/builtin/fn_macro.rs'
        with tarfile.open(vendor.ARCHIVE, 'r:gz') as archive:
            original = archive.extractfile(f"rust-analyzer-{self.provenance['commit']}/{target}").read().decode()
        current = (vendor.TREE / target).read_text()
        start = original.index('fn concat_expand(')
        body_start = original.index('    let mut err = None;', start)
        body_end = original.index('    ExpandResult { value: quote!(span =>#text), err }', body_start)
        extracted = current.index('fn concat_value_inner(')
        current_start = current.index('    let mut err = None;', extracted)
        current_end = current.index('    (text, span, err)', current_start)
        expected = original[body_start:body_end]
        expected = expected.replace(
            '                    tt::LitKind::Integer | tt::LitKind::Float => {\n                        format_to!(text, "{}", it.text())\n                    }',
            '                    tt::LitKind::Integer | tt::LitKind::Float => match concat_number_text(&it) {\n                        Ok(value) => text.push_str(&value),\n                        Err(error) => {\n                            err.get_or_insert(error);\n                        }\n                    },', 1)
        expected = expected.replace(
            '                        format_to!(text, "-{}", it.text());',
            "                        match concat_number_text(&it) {\n                            Ok(value) => {\n                                text.push('-');\n                                text.push_str(&value);\n                            }\n                            Err(error) => {\n                                err.get_or_insert(error);\n                            }\n                        }", 1)
        self.assertEqual(expected, current[current_start:current_end])
        self.assertIn('pub fn concat_value_tokens(tt: &tt::TopSubtree, call_site: Span)', current)



if __name__ == '__main__':
    unittest.main()

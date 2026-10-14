"""Fixture closure regression from unchanged official rustc proc-macro cases."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOL = Path(__file__).with_name("import_rustc_tests.py")
CHECKOUT = Path(os.environ.get("LRR_RUSTC_CHECKOUT", str(TOOL.parents[1] / "rustc/rust")))

class RustcImportTests(unittest.TestCase):
    def test_nested_auxiliary_dependencies_keep_original_test_root(self):
        spec = importlib.util.spec_from_file_location("rustc_fixture_import", TOOL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory(prefix="lrr-rustc-import-") as directory:
            destination = Path(directory) / "official"
            cases = ["tests/ui/proc-macro/mixed-site-span.rs", "tests/ui/proc-macro/resolved-located-at.rs"]
            with patch.object(module, "DESTINATION", destination), patch.object(sys, "argv", [str(TOOL), "--checkout", str(CHECKOUT), *cases]), contextlib.redirect_stdout(io.StringIO()):
                module.main()
            manifest = json.loads((destination / "manifest.json").read_text())
            files = {entry["path"]: entry for entry in manifest["files"]}
            self.assertIn("tests/ui/proc-macro/auxiliary/token-site-span.rs", files)
            self.assertIn("tests/ui/proc-macro/auxiliary/mixed-site-span.rs", files)
            self.assertIn("tests/ui/proc-macro/mixed-site-span.stderr", files)
            self.assertIn("tests/ui/proc-macro/resolved-located-at.stderr", files)
            self.assertFalse(any("auxiliary/auxiliary" in name for name in files))
            self.assertTrue({"LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"} <= files.keys())
            for name, entry in files.items():
                data = (destination / name).read_bytes()
                self.assertEqual(data, (CHECKOUT / name).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), entry["sha256"])
            for case in manifest["cases"]:
                self.assertEqual(case["lrr_acceptance"], "pending")
                self.assertEqual(case["native_acceptance"], "unexecuted")
                self.assertTrue(case["directives"])

    def test_official_include_helpers_are_support_files_not_cases(self):
        spec = importlib.util.spec_from_file_location("rustc_include_fixture_import", TOOL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        case = "tests/ui/macros/include-single-expr.rs"
        helpers = ["tests/ui/macros/include-single-expr-helper.rs",
                   "tests/ui/macros/include-single-expr-helper-1.rs"]
        with tempfile.TemporaryDirectory(prefix="lrr-rustc-include-import-") as directory:
            destination = Path(directory) / "fixtures/upstream/rustc-checkout"
            argv = [str(TOOL), "--checkout", str(CHECKOUT), case]
            for helper in helpers:
                argv.extend(["--support-file", helper])
            with patch.object(module, "DESTINATION", destination), patch.object(sys, "argv", argv), contextlib.redirect_stdout(io.StringIO()):
                module.main()
            manifest = json.loads((destination / "manifest.json").read_text())
            self.assertEqual([row["path"] for row in manifest["cases"]], [case])
            expected = {case, case[:-3] + ".stderr", *helpers,
                        "LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"}
            self.assertEqual({row["path"] for row in manifest["files"]}, expected)
            for row in manifest["files"]:
                original = (CHECKOUT / row["path"]).read_bytes()
                self.assertEqual((destination / row["path"]).read_bytes(), original)
                self.assertEqual(hashlib.sha256(original).hexdigest(), row["sha256"])
                blob = hashlib.sha1(b"blob " + str(len(original)).encode() + b"\0" + original).hexdigest()
                self.assertEqual(blob, row["git_blob"])
            # A later import of the primary cannot silently drop its retained helpers.
            with patch.object(module, "DESTINATION", destination), patch.object(sys, "argv", [str(TOOL), "--checkout", str(CHECKOUT), case]), contextlib.redirect_stdout(io.StringIO()):
                module.main()
            merged = json.loads((destination / "manifest.json").read_text())
            self.assertEqual(merged, manifest)

    def test_official_edition_macro_rustfix_snapshot_is_retained(self):
        spec = importlib.util.spec_from_file_location("rustc_rustfix_fixture_import", TOOL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        case = "tests/ui/macros/expr_2021_cargo_fix_edition.rs"
        with tempfile.TemporaryDirectory(prefix="lrr-rustc-rustfix-import-") as directory:
            destination = Path(directory) / "fixtures/upstream/rustc-checkout"
            with patch.object(module, "DESTINATION", destination), patch.object(sys, "argv", [str(TOOL), "--checkout", str(CHECKOUT), case]), contextlib.redirect_stdout(io.StringIO()):
                module.main()
            manifest = json.loads((destination / "manifest.json").read_text())
            self.assertEqual([row["path"] for row in manifest["cases"]], [case])
            expected = {case, case[:-3] + ".stderr", case[:-3] + ".fixed",
                        "LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"}
            self.assertEqual({row["path"] for row in manifest["files"]}, expected)
            self.assertEqual(manifest["cases"][0]["directives"],
                             ["//@ run-rustfix", "//@ check-pass", "//@ edition: 2021"])
            for row in manifest["files"]:
                original = (CHECKOUT / row["path"]).read_bytes()
                self.assertEqual((destination / row["path"]).read_bytes(), original)
                self.assertEqual(hashlib.sha256(original).hexdigest(), row["sha256"])
                blob = hashlib.sha1(b"blob " + str(len(original)).encode() + b"\0" + original).hexdigest()
                self.assertEqual(blob, row["git_blob"])

if __name__ == "__main__":
    unittest.main()

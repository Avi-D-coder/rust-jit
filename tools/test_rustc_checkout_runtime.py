"""Capture-integrity controls, not interpreter or upstream fixture execution."""
import copy
import json
from pathlib import Path
import unittest

import rustc_checkout_runtime as runner


class ManagedCapture(unittest.TestCase):
    def setUp(self):
        manifest = json.loads((runner.SOURCE / "manifest.json").read_text())
        self.files = {row["path"]: row for row in manifest["files"]}
        path = "tests/ui/autoref-autoderef/auto-ref.rs"
        case = next(row for row in manifest["cases"] if row["path"] == path)
        data = runner.checked_file(path, self.files)
        self.selected = {"case": path, **runner.contract(case, data, self.files)}
        self.digests = {path: {"content": "f" * 64}}
        self.context = "a" * 64
        self.observation = {"exit_code": 0, "stderr_bytes": 0, "stdout_bytes": 1000,
                            "timed_out": False, "output_limited": False, "pid": 17}
        self.packet = {
            "schema": "lrr/v5/standalone-runtime-capture/v1",
            "request_context": self.context, "process_id": 17,
            "transport_stdout_role": "protocol_json_not_guest_stdout",
            "host_preparation_streams": "outside_guest_capture_no_macro_host_or_build_output_promotion",
            "snapshot": {"owning_edition": 2015, "root_source": "auto-ref.rs", "files": [
                {"path": "auto-ref.rs", "bytes": len(data), "content": "f" * 64}]},
            "entry_body": {"definition": "b" * 64, "exact_body_bytes": "c" * 64},
            "terminal": {"live_edition": 2015, "schema": "lrr/v2/development-run",
                         "semantics_profile": "lrr-2024-v2", "status": "completed",
                         "value": {"kind": "unit"}, "output": {"stdout": "3\n", "stderr": ""}},
            "capture": {"complete": True, "blocker": None,
                        "scope": "dispatcher_external_effects_only_not_memory_trace",
                        "guest": {"status": "closed_managed_output_observed", "stdout": [51, 10], "stderr": []},
                        "facts": {"vm_dispatches": 3, "vm_outcomes": 3,
                                  "terminal": {"kind": "completed", "unit": True}, "cleanup_observed": True,
                                  "closed_no_external_effects": True, "observed_fault": False,
                                  "observed_suspension": False, "effect_recipe_dispatch_attempts": 0,
                                  "unsupported_operation_candidates": 0, "typed_fault_exit_candidates": 0}},
        }

    def classify(self, packet):
        return runner.classify(self.observation, packet, self.selected, self.context,
                               self.files, self.digests)

    def test_complete_observation_records_output_without_inventing_an_upstream_oracle(self):
        result = self.classify(self.packet)
        self.assertEqual(result["status"], "runtime_contract_pass")
        self.assertEqual(result["guest_output"], {"stdout": "3\n", "stderr": ""})
        self.assertFalse(self.selected["runtime_expected_files_asserted"])

    def test_altered_bytes_terminal_disagreement_and_unclosed_streams_never_pass(self):
        mutations = [
            lambda p: p["capture"]["guest"].update(stdout=[52, 10]),
            lambda p: p["terminal"]["output"].update(stdout="4\n"),
            lambda p: p["capture"]["guest"].update(status="open"),
            lambda p: p["capture"]["guest"].update(stdout=[True, 10]),
            lambda p: p["capture"]["guest"].update(stdout=[256]),
            lambda p: p["capture"]["guest"].update(stderr=[255]),
            lambda p: p["capture"].update(complete=False),
            lambda p: p["capture"]["facts"].update(cleanup_observed=False),
            lambda p: p["capture"]["facts"].update(effect_recipe_dispatch_attempts=1),
            lambda p: p.update(request_context="d" * 64),
        ]
        for mutate in mutations:
            packet = copy.deepcopy(self.packet)
            mutate(packet)
            self.assertNotEqual(self.classify(packet)["status"], "runtime_contract_pass")

    def test_omitted_empty_output_matches_serde_but_never_hides_captured_bytes(self):
        packet = copy.deepcopy(self.packet)
        del packet["terminal"]["output"]
        self.assertEqual(self.classify(packet)["status"], "incomplete_evidence")
        packet["capture"]["guest"].update(
            status="closed_no_external_effects_observed", stdout=[], stderr=[])
        self.assertEqual(self.classify(packet)["status"], "runtime_contract_pass")
        packet["terminal"]["output"] = None
        self.assertEqual(self.classify(packet)["status"], "incomplete_evidence")

    def test_empty_and_unicode_output_keep_exact_byte_status_contract(self):
        for text in ["", "π🙂\n"]:
            packet = copy.deepcopy(self.packet)
            packet["terminal"]["output"]["stdout"] = text
            packet["capture"]["guest"]["stdout"] = list(text.encode("utf-8"))
            packet["capture"]["guest"]["status"] = (
                "closed_managed_output_observed" if text else "closed_no_external_effects_observed")
            self.assertEqual(self.classify(packet)["status"], "runtime_contract_pass")


if __name__ == "__main__":
    unittest.main()

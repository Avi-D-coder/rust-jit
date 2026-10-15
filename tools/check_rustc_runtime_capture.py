"""Refusal controls derived from actual successful interpreter capture packets.

These are harness evidence controls, never additional upstream fixture passes.
Run after rustc_checkout_runtime.py against its unchanged validation workspace.
"""
import argparse
import copy
import json
from pathlib import Path

import rustc_checkout_runtime as runtime


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = json.loads(args.report.read_bytes())
    manifest_bytes = (runtime.SOURCE / "manifest.json").read_bytes()
    if runtime.sha(manifest_bytes) != report["manifest_sha256"]:
        raise ValueError("source inventory differs from observed run")
    files = {row["path"]: row for row in json.loads(manifest_bytes)["files"]}
    digests = json.loads((args.report.parent / "source-digests/transport.stdout").read_bytes())["files"]
    controls = []
    for ordinal, selected in enumerate(report["selected_cases"]):
        if selected["status"] != "runtime_contract_pass":
            continue
        raw = (args.report.parent / str(ordinal) / "transport.stdout").read_bytes()
        if runtime.sha(raw) != selected["observation"]["stdout_sha256"]:
            raise ValueError("actual capture bytes differ from receipt")
        packet = json.loads(raw)
        context = selected["capture_context"]
        observation = selected["observation"]
        if runtime.classify(observation, packet, selected, context, files, digests)["status"] != "runtime_contract_pass":
            raise ValueError("actual successful capture is no longer admissible")
        mutations = [
            ("wrong_protocol", ("schema",), "unknown"),
            ("wrong_request", ("request_context",), "0" * 64),
            ("wrong_process", ("process_id",), -1),
            ("wrong_edition", ("snapshot", "owning_edition"), 0),
            ("missing_sources", ("snapshot", "files"), []),
            ("wrong_source_digest", ("snapshot", "files", 0, "content"), "0" * 64),
            ("incomplete_capture", ("capture", "complete"), False),
            ("missing_stream_observation", ("capture", "guest", "stdout"), None),
            ("zero_dispatch", ("capture", "facts", "vm_dispatches"), 0),
            ("missing_cleanup", ("capture", "facts", "cleanup_observed"), False),
            ("integer_is_not_unit_bool", ("capture", "facts", "terminal", "unit"), 1),
            ("unobserved_effect", ("capture", "facts", "effect_recipe_dispatch_attempts"), 1),
            ("interpreter_fault", ("terminal", "status"), "fault"),
            ("unexpected_panic", ("terminal", "status"), "guest_panic"),
        ]
        for name, path, value in mutations:
            changed = copy.deepcopy(packet)
            node = changed
            for key in path[:-1]:
                node = node[key]
            node[path[-1]] = value
            outcome = runtime.classify(observation, changed, selected, context, files, digests)
            if outcome["status"] == "runtime_contract_pass":
                raise ValueError("accepted invalid evidence: " + name)
            controls.append({"case": selected["case"], "control": name, "status": outcome["status"]})
        for key, value in [("timed_out", True), ("output_limited", True), ("exit_code", 9)]:
            changed = dict(observation, **{key: value})
            outcome = runtime.classify(changed, packet, selected, context, files, digests)
            if outcome["status"] == "runtime_contract_pass":
                raise ValueError("accepted invalid transport: " + key)
            controls.append({"case": selected["case"], "control": key, "status": outcome["status"]})
    if not controls:
        raise ValueError("no actual runtime-contract pass available for controls")
    print(json.dumps({"schema": "lrr/v5/runtime-capture-controls/v1", "controls": controls,
                      "passed": True, "upstream_fixture_passes_added": 0}, indent=2))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""LRR route controls around one pinned rustc program; originals stay unchanged.

The handwritten row runs the exact imported original. Other rows inject only
macro wrappers around its first array-repeat expression, outside imported files.
This is runtime/compositionality evidence, not upstream compiletest acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "tests/ui/array-slice-vec/repeated-vector-syntax.rs"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/lrr")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    imported = ROOT / "fixtures/upstream/rustc-checkout" / FIXTURE
    manifest = json.loads((imported.parents[3] / "manifest.json").read_text())
    entry = next(row for row in manifest["files"] if row["path"] == FIXTURE)
    assert imported.read_bytes() == (ROOT / "rustc/rust" / FIXTURE).read_bytes()
    assert sha(imported) == entry["sha256"]
    original = imported.read_text()
    assert [line for line in original.splitlines() if line.startswith("//@")] == ["//@ run-pass"]
    operand = "[ [true]; 512 ]"
    assert original.count(operand) == 1
    producer = "macro_rules! operand { ($e:expr) => { $e }; }\n"
    wrapper = "macro_rules! wrapper { ($e:expr) => { operand!($e) }; }\n"
    sources = {
        "handwritten": original,
        "user_macro": producer + original.replace(operand, "operand!(" + operand + ")"),
        "wrapper_macro": producer + wrapper + original.replace(operand, "wrapper!(" + operand + ")"),
        "standard_macro": original.replace(
            operand,
            "{ assert!({ let probe = " + operand + "; probe[511_usize][0_usize] }); " + operand + " }",
        ),
    }
    expected_stdout = "[" + "[true], " * 512 + "]\n[0]\n"
    expected_binary = sha(binary)
    results = []
    environment = os.environ.copy()
    environment["LRR_EMBEDDED_FACADE"] = "1"
    for origin, source in sources.items():
        path = imported
        if origin != "handwritten":
            path = out / (origin + ".rs")
            path.write_text(source)
        for edition in (2015, 2018, 2021, 2024):
            assert sha(binary) == expected_binary
            label = f"{origin}-{edition}"
            command = [str(binary), "--edition", str(edition), "run", str(path), "--runtime-capture", "--capture-context", hashlib.sha256(label.encode()).hexdigest()]
            start = time.monotonic()
            with (out / (label + ".stdout")).open("wb") as stdout, (out / (label + ".stderr")).open("wb") as stderr:
                completed = subprocess.run(command, stdout=stdout, stderr=stderr, env=environment)
            seconds = time.monotonic() - start
            try:
                data = json.loads((out / (label + ".stdout")).read_text())
            except json.JSONDecodeError:
                data = {}
            terminal = data.get("terminal", {})
            output = terminal.get("output", {})
            runtime_pass = completed.returncode == 0 and data.get("capture", {}).get("complete") is True
            effects_match = output.get("stdout") == expected_stdout and output.get("stderr") == ""
            row = {
                "fixture": FIXTURE, "fixture_sha256": entry["sha256"], "upstream_commit": manifest["commit"],
                "origin": origin, "edition": edition, "unchanged_original": origin == "handwritten",
                "adapter_sha256": hashlib.sha256(source.encode()).hexdigest(), "binary_sha256": expected_binary,
                "exit": completed.returncode, "seconds": round(seconds, 3),
                "runtime_contract_pass": runtime_pass, "expected_effects_match": effects_match,
                "pass": runtime_pass and effects_match,
                "terminal_status": terminal.get("status"), "fault_code": terminal.get("code"), "fault_message": terminal.get("message"),
                "array_repeat_operations": terminal.get("opcode_counters", {}).get("started_by_opcode", [None] * 64)[63],
                "capture_facts": data.get("capture", {}).get("facts"),
                "raw_stdout_sha256": sha(out / (label + ".stdout")), "argv": command,
            }
            results.append(row)
            (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            assert sha(binary) == expected_binary
            print(json.dumps({k: row[k] for k in ("origin", "edition", "exit", "seconds", "pass", "fault_code", "fault_message")}), flush=True)
    return 0 if all(row["pass"] for row in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare a pinned generic reference-coercion main across source origins.

Keep the upstream original unchanged. Adaptations wrap its complete actual main
body outside the licensed import, retaining its own assertions and call order.
This is route/runtime evidence, not a full macro architecture acceptance gate.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "tests/ui/regions/regions-infer-borrow-scope-view.rs"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/lrr")
    args = parser.parse_args()
    binary, out = args.binary.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    original = ROOT / "fixtures/upstream/rustc-checkout" / FIXTURE
    manifest = json.loads((ROOT / "fixtures/upstream/rustc-checkout/manifest.json").read_text())
    entry = next(row for row in manifest["files"] if row["path"] == FIXTURE)
    assert sha(original) == entry["sha256"]
    assert original.read_bytes() == (ROOT / "rustc/rust" / FIXTURE).read_bytes()
    text = original.read_text()
    assert [line for line in text.splitlines() if line.startswith("//@")] == ["//@ run-pass"]
    opening = "pub fn main() {"
    assert text.count(opening) == 1
    prefix, body = text.split(opening)
    assert body.rstrip().endswith("}")
    body = "{" + body.rstrip()[:-1] + "}"
    producer = "macro_rules! emit { ($e:expr) => { $e }; }\n"
    wrapper = "macro_rules! wrapper { ($e:expr) => { emit!($e) }; }\n"
    adaptations = {
        "user_macro": producer + prefix + opening + "emit!(" + body + ");}",
        "wrapper_macro": producer + wrapper + prefix + opening + "wrapper!(" + body + ");}",
        "standard_macro": prefix + opening + "assert!(matches!(" + body + ", ()));}",
    }
    expected_binary = sha(binary)
    environment = os.environ.copy()
    environment["LRR_EMBEDDED_FACADE"] = "1"
    results = []
    for origin, source in adaptations.items():
        path = out / (origin + ".rs")
        path.write_text(source)
        for edition in (2015, 2018, 2021, 2024):
            assert sha(binary) == expected_binary
            name = f"{origin}-{edition}"
            context = hashlib.sha256((name + expected_binary).encode()).hexdigest()
            command = [str(binary), "--edition", str(edition), "run", str(path),
                       "--runtime-capture", "--capture-context", context]
            start = time.monotonic()
            with (out / (name + ".stdout")).open("wb") as stdout, (out / (name + ".stderr")).open("wb") as stderr:
                completed = subprocess.run(command, stdout=stdout, stderr=stderr, env=environment)
            try:
                data = json.loads((out / (name + ".stdout")).read_text())
            except json.JSONDecodeError:
                data = {}
            capture = data.get("capture") or {}
            facts = capture.get("facts") or {}
            guest = capture.get("guest") or {}
            row = {
                "fixture": FIXTURE, "fixture_sha256": entry["sha256"],
                "upstream_commit": manifest["commit"], "origin": origin, "edition": edition,
                "unchanged_original": False, "adapter_sha256": hashlib.sha256(source.encode()).hexdigest(),
                "binary_sha256": expected_binary, "harness_sha256": sha(Path(__file__)),
                "argv": command, "exit": completed.returncode, "seconds": round(time.monotonic() - start, 3),
                "runtime_contract_pass": completed.returncode == 0 and capture.get("complete") is True,
                "cleanup_observed": facts.get("cleanup_observed") is True,
                "no_output": guest.get("stdout") == [] and guest.get("stderr") == [],
                "source_call_boundaries": facts.get("source_call_boundaries"),
                "terminal": {key: (data.get("terminal") or {}).get(key) for key in ("status", "code", "message")},
            }
            row["pass"] = row["runtime_contract_pass"] and row["cleanup_observed"] and row["no_output"]
            results.append(row)
            (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(json.dumps(row), flush=True)
            assert sha(binary) == expected_binary
    return 0 if all(row["pass"] for row in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())

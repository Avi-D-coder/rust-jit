#!/usr/bin/env python3
"""LRR exact-once/coldness controls around the pinned unchanged const-repeat main.

Adaptations wrap one expression outside imported originals. Successful process
exit alone is insufficient: require complete interpreter capture, no output,
no panic, and the ordinary bar/Vec::new call count (one each).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "tests/ui/consts/const-blocks/const-repeat.rs"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/lrr")
    parser.add_argument("--only-handwritten", action="store_true")
    args = parser.parse_args()
    binary, out = args.binary.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    original = ROOT / "fixtures/upstream/rustc-checkout" / FIXTURE
    manifest = json.loads((ROOT / "fixtures/upstream/rustc-checkout/manifest.json").read_text())
    entry = next(r for r in manifest["files"] if r["path"] == FIXTURE)
    assert sha(original) == entry["sha256"]
    assert original.read_bytes() == (ROOT / "rustc/rust" / FIXTURE).read_bytes()
    source = original.read_text()
    assert [l for l in source.splitlines() if l.startswith("//@")] == ["//@ run-pass"]
    expression = "[EMPTY; 2]"
    assert source.count(expression) == 1
    producer = "macro_rules! emit { ($e:expr) => { $e }; }\n"
    wrapper = "macro_rules! wrapper { ($e:expr) => { emit!($e) }; }\n"
    sources = {
        "handwritten": source,
        "user_macro": producer + source.replace(expression, "emit!(" + expression + ")"),
        "wrapper_macro": producer + wrapper + source.replace(expression, "wrapper!(" + expression + ")"),
        "standard_macro": source.replace(expression, "if matches!(" + expression + ", [_, _]) { " + expression + " } else { " + expression + " }"),
    }
    if args.only_handwritten:
        sources = {"handwritten": source}
    binary_sha = sha(binary)
    environment = os.environ.copy()
    environment["LRR_EMBEDDED_FACADE"] = "1"
    results = []
    for origin, text in sources.items():
        path = original
        if origin != "handwritten":
            path = out / (origin + ".rs")
            path.write_text(text)
        for edition in (2015, 2018, 2021, 2024):
            assert sha(binary) == binary_sha
            name = f"{origin}-{edition}"
            command = [str(binary), "--edition", str(edition), "run", str(path), "--runtime-capture", "--capture-context", hashlib.sha256(name.encode()).hexdigest()]
            start = time.monotonic()
            with (out / (name + ".stdout")).open("wb") as stdout, (out / (name + ".stderr")).open("wb") as stderr:
                completed = subprocess.run(command, stdout=stdout, stderr=stderr, env=environment)
            try:
                data = json.loads((out / (name + ".stdout")).read_text())
            except json.JSONDecodeError:
                data = {}
            terminal = data.get("terminal") or {}
            counters = terminal.get("opcode_counters", {}).get("started_by_opcode", [])
            calls = counters[19] if len(counters) > 19 else None
            runtime_pass = completed.returncode == 0 and data.get("capture", {}).get("complete") is True
            guest = data.get("capture", {}).get("guest") or {}
            quiet = guest.get("stdout") == [] and guest.get("stderr") == []
            initializer_frames = (data.get("capture", {}).get("facts") or {}).get("initializer_frames_entered")
            row = {
                "fixture": FIXTURE, "fixture_sha256": entry["sha256"], "upstream_commit": manifest["commit"],
                "origin": origin, "edition": edition, "unchanged_original": origin == "handwritten",
                "adapter_sha256": hashlib.sha256(text.encode()).hexdigest(), "binary_sha256": binary_sha,
                "exit": completed.returncode, "seconds": round(time.monotonic() - start, 3),
                "runtime_contract_pass": runtime_pass, "no_output": quiet,
                "expected_calls": 2, "observed_calls": calls, "initializer_once": calls == 2,
                "initializer_frames_entered": initializer_frames, "expected_initializer_frames": 1,
                "pass": runtime_pass and quiet and calls == 2 and initializer_frames == 1,
                "fault_code": terminal.get("code"), "fault_message": terminal.get("message"),
                "capture_facts": data.get("capture", {}).get("facts"),
                "raw_stdout_sha256": sha(out / (name + ".stdout")), "argv": command,
            }
            results.append(row)
            (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            assert sha(binary) == binary_sha
            print(json.dumps({k: row[k] for k in ("origin", "edition", "exit", "runtime_contract_pass", "observed_calls", "initializer_frames_entered", "pass", "fault_code", "fault_message")}), flush=True)
    return 0 if all(r["pass"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())

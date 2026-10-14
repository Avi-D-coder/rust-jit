"""Run selected unchanged rustc cases through LRR, with explicit native references.

This is a restricted interpreter harness, not rustc compiletest or JIT acceptance.
Run after building LRR with pinned Cargo: python3 tools/rustc_checkout_interpreter.py
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/upstream/rustc-checkout"
CASE = "tests/ui/derives/deriving-meta-empty-trait-list.rs"
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"


def verified_case(manifest, directory):
    if manifest["commit"] != COMMIT:
        raise ValueError("unreviewed upstream revision")
    case = next(c for c in manifest["cases"] if c["path"] == CASE)
    data = (directory / CASE).read_bytes()
    entry = next(f for f in manifest["files"] if f["path"] == CASE)
    blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
    if (len(data), hashlib.sha256(data).hexdigest(), blob) != (
        entry["bytes"], entry["sha256"], entry["git_blob"]
    ):
        raise ValueError("copied upstream case differs from its provenance")
    directives = [line.strip() for line in data.decode().splitlines()
                  if line.lstrip().startswith("//@")]
    if directives != ["//@ check-pass"] or case["directives"] != directives:
        raise ValueError("unsupported or changed compiletest directives")
    return entry


def command(argv):
    result = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True,
                            timeout=60, check=False)
    return {"argv": list(map(str, argv)), "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr}


def completed_interpreter(observation):
    if observation["exit_code"] != 0 or observation["stderr"]:
        return False
    payload = json.loads(observation["stdout"])
    counters = payload["opcode_counters"]
    return (payload["schema"] == "lrr/v2/development-run"
            and payload["semantics_profile"] == "lrr-2024-v2"
            and payload["live_edition"] == 2024
            and payload["status"] == "completed"
            and payload["value"] == {"kind": "unit"}
            and counters["operations_completed"] > 0
            and counters["operations_started"] == counters["operations_completed"]
            and counters["panic_scopes_cleaned"] == 0
            and payload["installed_recipes"] > 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    # Developer validation shares Cargo's configured host target directory.
    # A relative CARGO_TARGET_DIR is interpreted from this harness's Cargo cwd.
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    if not target.is_absolute():
        target = ROOT / target
    parser.add_argument("--lrr", type=Path, default=target / "debug/lrr")
    args = parser.parse_args()
    binary = args.lrr.resolve(strict=True)
    manifest_bytes = (SOURCE / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    entry = verified_case(manifest, SOURCE)
    # Verify retained licensing as well as the selected source.
    for name in ["LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"]:
        row = next(f for f in manifest["files"] if f["path"] == name)
        if hashlib.sha256((SOURCE / name).read_bytes()).hexdigest() != row["sha256"]:
            raise ValueError("copied license differs from upstream provenance")
    version = command(["rustc", "+1.99.0", "-vV"])
    if (version["exit_code"] != 0
            or "release: 1.99.0\n" not in version["stdout"]
            or "host: aarch64-apple-darwin\n" not in version["stdout"]):
        raise ValueError("requires pinned Rust 1.99.0 macOS ARM64 reference compiler")
    directory = Path(tempfile.mkdtemp(prefix="rustc-checkout-interpreter-",
                                     dir=ROOT / "artifacts/macos-first"))
    report = {
        "schema_version": 1, "task": "M013", "upstream_commit": COMMIT,
        "platform": "aarch64-apple-darwin", "edition": 2024,
        "scope": "Unchanged check-pass source: native metadata reference and LRR CLI interpreter",
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "source": entry, "compiler": version,
        "lrr_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "native_check_cases_passed": [], "interpreter_cases_passed": [],
        "full_compiletest_cases_passed": [], "lrr_full_cases_passed": [],
        "guest_jit_acceptance": False, "release_complete": False,
    }
    passed = False
    try:
        # Explicit developer reference preparation; LRR never invokes this compiler.
        native = command(["rustc", "+1.99.0", str(SOURCE / CASE), "--edition=2024",
                          "--emit=metadata", "-o", str(directory / "reference.rmeta")])
        report["native_check"] = native
        if native["exit_code"] == 0 and (directory / "reference.rmeta").is_file():
            report["native_check_cases_passed"] = [CASE]
        observation = command([str(binary), "run", str(SOURCE / CASE)])
        report["interpreter"] = observation
        if completed_interpreter(observation):
            report["interpreter_cases_passed"] = [CASE]
        # Detect source/binary substitution during the run as well.
        verified_case(manifest, SOURCE)
        if hashlib.sha256(binary.read_bytes()).hexdigest() != report["lrr_binary_sha256"]:
            raise ValueError("LRR binary changed during observation")
        passed = bool(report["native_check_cases_passed"] and report["interpreter_cases_passed"])
    finally:
        report["selected_scope_passed"] = passed
        path = directory / "report.json"
        path.write_text(json.dumps(report, indent=2) + "\n")
        print(path.relative_to(ROOT))
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

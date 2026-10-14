"""Explicit native reference preparation for the imported rustc assessment cases.

This does not invoke LRR, exercise a query provider, or certify a ledger task.
Run from any directory with: python3 tools/rustc_upstream_oracles.py
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/upstream/rustc"
MANIFEST = json.loads((SOURCE / "manifest.json").read_text())
COMPILER = ["rustc", "+1.99.0"]


def command(argv):
    result = subprocess.run(
        argv, cwd=ROOT, capture_output=True, text=True, timeout=60, check=False
    )
    return {
        "argv": list(map(str, argv)),
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }


def main():
    for entry in MANIFEST["files"]:
        data = (SOURCE / entry["path"]).read_bytes()
        assert len(data) == entry["bytes"]
        assert hashlib.sha256(data).hexdigest() == entry["sha256"]
        assert hashlib.sha1(
            b"blob " + str(len(data)).encode() + b"\0" + data
        ).hexdigest() == entry["git_blob"]
    version = command(COMPILER + ["-vV"])
    assert version["exit_code"] == 0
    assert f"commit-hash: {MANIFEST['commit']}\n" in version["stdout"]
    assert "host: aarch64-apple-darwin\n" in version["stdout"]
    artifact_root = ROOT / "artifacts/macos-first"
    directory = Path(tempfile.mkdtemp(prefix="rustc-upstream-", dir=artifact_root))
    (directory / "manifest.json").write_bytes((SOURCE / "manifest.json").read_bytes())
    observations = []
    for case in MANIFEST["cases"]:
        if case["mode"] == "compile-fail":
            # Compiler rejection is not a native expected-pass or LRR acceptance.
            continue
        assert case["mode"] in ("run-pass", "build-pass", "check-pass")
        source = SOURCE / case["path"]
        output = directory / source.stem
        argv = COMPILER + [
            str(source), "--edition=2015", "-C", "opt-level=0",
            "-C", "overflow-checks=yes", "-C", "debug-assertions=yes",
            "-C", "panic=unwind", "-o", str(output),
        ]
        if case["mode"] == "check-pass":
            argv += ["--emit=metadata"]
        compiled = command(argv)
        observation = {"path": case["path"], "mode": case["mode"], "compile": compiled}
        observations.append(observation)
        if compiled["exit_code"] != 0:
            break
        if case["mode"] == "run-pass":
            observation["run"] = command([str(output)])
            if observation["run"]["exit_code"] != 0:
                break
    selected = [c for c in MANIFEST["cases"] if c["mode"] != "compile-fail"]
    passed = len(observations) == len(selected) and all(
        o["compile"]["exit_code"] == 0
        and (o["mode"] != "run-pass" or o["run"]["exit_code"] == 0)
        for o in observations
    )
    report = {
        "schema_version": 1,
        "upstream_commit": MANIFEST["commit"],
        "manifest_sha256": hashlib.sha256((SOURCE / "manifest.json").read_bytes()).hexdigest(),
        "compiler": version,
        "edition": 2015,
        "scope": "Native reference preparation only; no LRR test or task acceptance",
        "observations": observations,
        "native_selected_passed": passed,
        "diagnostic_cases_executed": [],
        "lrr_tests_passed": [],
        "release_complete": False,
    }
    path = directory / "report.json"
    path.write_text(json.dumps(report, indent=2) + "\n")
    print(path.relative_to(ROOT))
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

"""Execute five unchanged official arithmetic programs and their original assertions.

Developer-only native reference preparation is explicit. The LRR CLI never builds
these units. The original compiletest edition2015 and an edition2024 reference are
separate runs; unchanged bytes under LRR2024 are an explicit edition adaptation.
This does not establish full compiletest, JIT, release or Linux acceptance.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from rustc_checkout_interpreter import completed_interpreter

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/upstream/rustc-checkout"
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
CASES = tuple("tests/ui/numbers-arithmetic/" + name + ".rs" for name in (
    "i32-sub", "arith-unsigned", "floatlits", "isize-base", "integer-literal-radix",
))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def verified_inputs():
    raw = (SOURCE / "manifest.json").read_bytes()
    manifest = json.loads(raw)
    if manifest["commit"] != COMMIT or manifest["license"] != "MIT OR Apache-2.0":
        raise ValueError("unreviewed upstream revision or license")
    rows = {row["path"]: row for row in manifest["files"]}
    cases = {row["path"]: row for row in manifest["cases"]}
    selected = {}
    for name in (*CASES, "LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"):
        row = rows[name]
        data = (SOURCE / name).read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if (len(data), sha(data), blob) != (row["bytes"], row["sha256"], row["git_blob"]):
            raise ValueError("changed original input: " + name)
        if name in CASES:
            directives = [line.strip() for line in data.decode().splitlines()
                          if line.lstrip().startswith("//@")]
            if directives != ["//@ run-pass"] or cases[name]["directives"] != directives:
                raise ValueError("unsupported or changed original directives: " + name)
            # i32-sub places its assertion after an assignment on its single
            # main line. Count all actual assertion spellings in these fixed
            # originals, which contain no quoted/commented assertion text.
            assertions = data.decode().count("assert!(") + data.decode().count("assert_eq!(")
            if assertions == 0:
                raise ValueError("selected original has no assertion: " + name)
            selected[name] = dict(row, source_assertions=assertions)
    return sha(raw), selected


def command(argv):
    try:
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, capture_output=True,
                                text=True, timeout=60, check=False)
        return {"argv": list(map(str, argv)), "exit_code": result.returncode,
                "timed_out": False, "stdout": result.stdout, "stderr": result.stderr}
    except subprocess.TimeoutExpired as error:
        def text(value):
            return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
        return {"argv": list(map(str, argv)), "exit_code": None, "timed_out": True,
                "stdout": text(error.stdout), "stderr": text(error.stderr)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lrr", type=Path, required=True)
    parser.add_argument("--report-dir", type=Path, required=True,
                        help="New external directory; must be outside the frozen candidate")
    parser.add_argument("--native-references-authorized", action="store_true", required=True)
    args = parser.parse_args()
    binary = args.lrr.resolve(strict=True)
    directory = args.report_dir.resolve()
    if directory.is_relative_to(ROOT) or directory.exists():
        raise ValueError("report directory must be new and outside the frozen candidate")
    manifest_hash, sources = verified_inputs()
    binary_hash = sha(binary.read_bytes())
    version = command(["rustc", "+1.99.0", "-vV"])
    if (version["exit_code"] != 0 or "release: 1.99.0\n" not in version["stdout"]
            or "host: aarch64-apple-darwin\n" not in version["stdout"]):
        raise ValueError("requires Rust1.99.0 macOS ARM64 reference compiler")
    directory.mkdir(parents=True)
    report = {
        "schema": 1, "task": "T041", "scope": "Affected pure lowering: original arithmetic assertions",
        "upstream_commit": COMMIT, "manifest_sha256": manifest_hash, "sources": sources,
        "compiler": version, "platform": "aarch64-apple-darwin",
        "lrr_binary": str(binary), "lrr_binary_sha256": binary_hash,
        "original_source_edition": 2015, "lrr_edition_adaptation": 2024,
        "native_reference_editions": [2015, 2024], "cases": {},
        "native_original_run_passes": [], "native_2024_run_passes": [],
        "interpreter_2024_whole_source_passes": [], "full_compiletest_passes": [],
        "full_lrr_fixture_passes": [], "guest_jit_acceptance": False,
        "release_complete": False, "linux_acceptance": False, "batch_passed": False,
    }
    try:
        for name in CASES:
            case = {"status": "running", "native_references": {}}
            report["cases"][name] = case
            for edition in [2015, 2024]:
                output = directory / (Path(name).stem + "-" + str(edition))
                native = case["native_references"][str(edition)] = {}
                native["compile"] = command([
                    "rustc", "+1.99.0", SOURCE / name, "--edition=" + str(edition), "-o", output,
                ])
                if native["compile"]["exit_code"] != 0 or not output.is_file():
                    case["status"] = "native_compile_failed"
                    break
                native["binary_sha256"] = sha(output.read_bytes())
                native["run"] = command([output])
                # These five original programs have assertions and no output.
                if (native["run"]["exit_code"] != 0 or native["run"]["stdout"]
                        or native["run"]["stderr"]):
                    case["status"] = "native_run_failed"
                    break
                key = "native_original_run_passes" if edition == 2015 else "native_2024_run_passes"
                report[key].append(name)
            if case["status"] != "running":
                # Preserve this failure and still observe every other selected
                # unchanged program. A failed case never enters the pass lists.
                continue
            observation = case["interpreter"] = command([binary, "run", SOURCE / name])
            try:
                payload = json.loads(observation["stdout"])
                started = payload["opcode_counters"]["started_by_opcode"][26]
                case["observed_assertions_started"] = started
                passed = (completed_interpreter(observation)
                          and type(started) is int
                          and started == sources[name]["source_assertions"])
            except (ValueError, KeyError, IndexError, TypeError):
                passed = False
            if not passed:
                case["status"] = "interpreter_failed"
                continue
            report["interpreter_2024_whole_source_passes"].append(name)
            case["status"] = "scoped_pass"
    except Exception as error:
        report["harness_error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        try:
            after_manifest_hash, after_sources = verified_inputs()
            report["source_and_binary_unchanged"] = (
                after_manifest_hash == manifest_hash and after_sources == sources
                and sha(binary.read_bytes()) == binary_hash
            )
        except (OSError, ValueError, KeyError) as error:
            report["source_and_binary_unchanged"] = False
            report["integrity_error"] = type(error).__name__ + ": " + str(error)
        report["batch_passed"] = (
            "harness_error" not in report
            and report["source_and_binary_unchanged"]
            and report["native_original_run_passes"] == list(CASES)
            and report["native_2024_run_passes"] == list(CASES)
            and report["interpreter_2024_whole_source_passes"] == list(CASES)
        )
        for name in CASES:
            report["cases"].setdefault(name, {"status": "unexecuted"})
        path = directory / "report.json"
        path.write_text(json.dumps(report, indent=2) + "\n")
        print(path)
    if not report["batch_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

"""Observe eight unchanged rustc run-fail programs in separate LRR processes.

T045 checks ordinary guest panic, retained source origin and completed bounded
scope cleanup. Native reference preparation needs explicit developer authority.
The CLI JSON contract is an edition2024/output adaptation: original stderr
error-pattern results are recorded separately and never count as compiletest
acceptance. A timeout, host crash or runtime fault cannot satisfy guest panic.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/upstream/rustc-checkout"
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
CASES = tuple("tests/ui/panics/" + name + ".rs" for name in (
    "panic-macro-explicit", "panic-parens", "while-body-panics", "panic-arg", "args-panic",
    "explicit-panic", "doublepanic", "panic",
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
        if name not in CASES:
            continue
        directives = [line.strip() for line in data.decode().splitlines()
                      if line.lstrip().startswith("//@")]
        patterns = []
        for directive in directives:
            if directive in ("//@ run-fail", "//@ needs-subprocess"):
                continue
            if directive.startswith("//@ error-pattern:"):
                pattern = directive.removeprefix("//@ error-pattern:").strip()
                if pattern:
                    patterns.append(pattern)
                    continue
            raise ValueError("unsupported original directive: " + directive)
        if (directives.count("//@ run-fail") != 1
                or directives.count("//@ needs-subprocess") != 1
                or not patterns or cases[name]["directives"] != directives):
            raise ValueError("changed or unsupported directive set: " + name)
        kind, opcode = ("assertion", 26) if name == "tests/ui/panics/panic.rs" else ("explicit", 25)
        selected[name] = dict(row, directives=directives, error_patterns=patterns,
                              expected_guest_panic_kind=kind, expected_panic_opcode=opcode)
    return sha(raw), selected


def command(argv):
    argv = list(map(str, argv))
    try:
        result = subprocess.run(argv, cwd=ROOT, capture_output=True,
                                text=True, timeout=60, check=False)
        return {"argv": argv, "exit_code": result.returncode,
                "timed_out": False, "stdout": result.stdout, "stderr": result.stderr}
    except subprocess.TimeoutExpired as error:
        def text(value):
            return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
        return {"argv": argv, "exit_code": None, "timed_out": True,
                "stdout": text(error.stdout), "stderr": text(error.stderr)}
    except OSError as error:
        return {"argv": argv, "exit_code": None, "timed_out": False,
                "stdout": "", "stderr": "", "launch_error": str(error)}


def integer(value, maximum):
    return type(value) is int and 0 <= value <= maximum


def panic_observation(observation, source):
    """Strictly classify the JSON adaptation; never treat any nonzero as panic."""
    if (observation["exit_code"] != 1 or observation["timed_out"]
            or observation["stderr"] or "launch_error" in observation):
        return {"passed": False, "reason": "not_clean_guest_panic_exit"}
    try:
        payload = json.loads(observation["stdout"])
        panic = payload["panic"]
        counters = payload["opcode_counters"]
        opcodes = counters["started_by_opcode"]
        pc = panic["pc"]
        span = panic["origin"]["span"]
        message = panic["message"]
        # `panicked` is native panic-hook framing. It is separately checked on
        # original stderr; JSON's guest_panic status supplies the adapted fact.
        message_patterns = [p for p in source["error_patterns"] if p != "panicked"]
        checks = {
            "schema": payload["schema"] == "lrr/v2/development-run",
            "profile": payload["semantics_profile"] == "lrr-2024-v2",
            "edition": type(payload["live_edition"]) is int and payload["live_edition"] == 2024,
            "ordinary_guest_panic": payload["status"] == "guest_panic"
            and panic["kind"] == source["expected_guest_panic_kind"],
            "original_message_patterns": isinstance(message, str)
            and bool(message_patterns) and all(p in message for p in message_patterns),
            "one_reached_panic": isinstance(opcodes, list) and len(opcodes) == 41
            and all(integer(n, 2**64 - 1) for n in opcodes)
            and opcodes[source["expected_panic_opcode"]] == 1
            and opcodes[26 if source["expected_panic_opcode"] == 25 else 25] == 0,
            "scopes_cleaned": integer(counters["panic_scopes_cleaned"], 2**64 - 1)
            and counters["panic_scopes_cleaned"] > 0,
            "actual_operations": integer(counters["operations_started"], 2**64 - 1)
            and integer(counters["operations_completed"], 2**64 - 1)
            and 0 < counters["operations_completed"] <= counters["operations_started"],
            "original_body_retained": bool(pc["body"]) and span["body_key"] == pc["body"],
            "checked_pc": integer(pc["node"], 2**32 - 1) and integer(pc["phase"], 2**16 - 1),
            "checked_source_span": integer(span["relative_start"], source["bytes"])
            and integer(span["relative_end"], source["bytes"])
            and span["relative_start"] < span["relative_end"],
            "origin_ancestry": isinstance(panic["origin"]["expansion_ancestry"], list),
        }
        return {"passed": all(checks.values()), "checks": checks, "payload": payload}
    except (ValueError, KeyError, IndexError, TypeError) as error:
        return {"passed": False, "reason": "invalid_guest_panic_report", "error": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lrr", type=Path, required=True)
    parser.add_argument("--report-dir", type=Path, required=True,
                        help="New external directory, outside the frozen source")
    parser.add_argument("--native-references-authorized", action="store_true", required=True)
    args = parser.parse_args()
    binary = args.lrr.resolve(strict=True)
    directory = args.report_dir.resolve()
    if directory.is_relative_to(ROOT) or directory.exists():
        raise ValueError("report directory must be new and outside the frozen candidate")
    manifest_hash, sources = verified_inputs()
    binary_hash = sha(binary.read_bytes())
    directory.mkdir(parents=True)
    report = {
        "schema": 1, "task": "T045", "scope": "Whole-source guest panic and completed scope cleanup",
        "upstream_commit": COMMIT, "manifest_sha256": manifest_hash, "sources": sources,
        "platform": "aarch64-apple-darwin", "lrr_binary": str(binary),
        "lrr_binary_sha256": binary_hash, "original_source_edition": 2015,
        "lrr_edition_adaptation": 2024, "native_reference_editions": [2015, 2024],
        "cases": {}, "native_original_run_fail_observations": [],
        "native_2024_run_fail_observations": [], "interpreter_2024_panic_semantics_observations": [],
        "full_compiletest_passes": [], "full_lrr_fixture_passes": [],
        "guest_jit_acceptance": False, "release_complete": False,
        "linux_acceptance": False, "batch_passed": False,
    }
    try:
        version = report["compiler"] = command(["rustc", "+1.99.0", "-vV"])
        if (version["exit_code"] != 0 or "release: 1.99.0\n" not in version["stdout"]
                or "host: aarch64-apple-darwin\n" not in version["stdout"]):
            raise ValueError("requires Rust1.99.0 macOS ARM64 reference compiler")
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
                    native["status"] = "compile_failed"
                    continue
                native["binary_sha256"] = sha(output.read_bytes())
                native["run"] = command([output])
                observed = native["run"]
                native["stderr_patterns"] = {
                    p: p in observed["stderr"] for p in sources[name]["error_patterns"]
                }
                passed = (observed["exit_code"] == 101 and not observed["timed_out"]
                          and not observed["stdout"] and all(native["stderr_patterns"].values()))
                native["status"] = "original_run_fail_observed" if passed else "run_failed"
                if passed:
                    key = ("native_original_run_fail_observations" if edition == 2015
                           else "native_2024_run_fail_observations")
                    report[key].append(name)
            # Always observe LRR, even when a native reference failed.
            observed = case["interpreter"] = command([binary, "run", SOURCE / name])
            classification = case["interpreter_classification"] = panic_observation(observed, sources[name])
            case["original_stderr_error_patterns"] = {
                p: p in observed["stderr"] for p in sources[name]["error_patterns"]
            }
            case["original_directive_observations"] = {
                "run_fail": classification["passed"],
                "needs_subprocess": "launch_error" not in observed,
                "stderr_error_patterns": all(case["original_stderr_error_patterns"].values()),
            }
            if classification["passed"]:
                report["interpreter_2024_panic_semantics_observations"].append(name)
                case["status"] = "adapted_panic_semantics_observed"
            elif observed["timed_out"]:
                case["status"] = "interpreter_timeout"
            elif type(observed["exit_code"]) is int and observed["exit_code"] < 0:
                case["status"] = "interpreter_host_crash"
            else:
                case["status"] = "interpreter_failed"
    except Exception as error:
        report["harness_error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        try:
            after_hash, after_sources = verified_inputs()
            report["source_and_binary_unchanged"] = (
                after_hash == manifest_hash and after_sources == sources
                and sha(binary.read_bytes()) == binary_hash
            )
        except (OSError, ValueError, KeyError) as error:
            report["source_and_binary_unchanged"] = False
            report["integrity_error"] = type(error).__name__ + ": " + str(error)
        report["batch_passed"] = (
            "harness_error" not in report and report["source_and_binary_unchanged"]
            and report["native_original_run_fail_observations"] == list(CASES)
            and report["native_2024_run_fail_observations"] == list(CASES)
            and report["interpreter_2024_panic_semantics_observations"] == list(CASES)
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

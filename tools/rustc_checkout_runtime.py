"""Execute unchanged, selected rustc runtime contracts through the LRR CLI.

No native compilation or reference execution occurs. This first adapter supports
plain run-pass plus owning-edition and aux-build/aux-crate directives (as
`--extern` source dependency crates with their own editions) and the existing
closed guest-effect capture. Other contracts/dependencies remain explicit harness blockers, not skips.
Selected runtime evidence never completes the entire upstream or diagnostic gates.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import secrets
import selectors
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/upstream/rustc-checkout"
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
MAX_OUTPUT = 8 * 1024 * 1024
HEX = re.compile(r"[0-9a-f]{64}\Z")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def checked_file(name, files):
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or str(path) != name:
        raise ValueError("noncanonical original path: " + name)
    location = (SOURCE / name).resolve(strict=True)
    if not location.is_relative_to(SOURCE.resolve()):
        raise ValueError("original path escapes licensed input directory")
    row = files[name]
    data = location.read_bytes()
    blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
    if (len(data), sha(data), blob) != (row["bytes"], row["sha256"], row["git_blob"]):
        raise ValueError("original bytes differ from pinned manifest: " + name)
    return data


AUX_CRATE = re.compile(r"(?:([a-z,]+):)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\S+)\Z")


def extern_flags(case_path, aux, files, seen=None, root=True):
    """compiletest auxiliaries as `lrr --extern` source dependency crates.

    `aux-build:FILE` is found by `extern crate` only (rustc `-L` search), so it
    becomes `--extern noprelude:NAME=PATH`; `aux-crate:[OPTS:]NAME=FILE` is a
    rustc `--extern`. Each auxiliary keeps its own `//@ edition` (compiletest
    default 2015). Nested auxiliaries resolve against the top-level test's
    `auxiliary/` directory and reach the root only through `extern crate`.
    """
    seen = set() if seen is None else seen
    flags, blockers = [], []
    base = PurePosixPath(case_path).parent / "auxiliary"
    for kind, value in aux:
        if kind == "aux-crate":
            match = AUX_CRATE.fullmatch(value)
            if not match:
                blockers.append("unparsed aux-crate directive: " + value)
                continue
            options, name, file = match.groups()
            prefix = options + ":" if options else ""
        else:
            file = value
            name = PurePosixPath(file).stem.replace("-", "_")
            prefix = "noprelude:"
        if not root:
            prefix = "noprelude:"
        original = str(base / file)
        if original not in files:
            blockers.append("auxiliary source is not imported: " + original)
            continue
        text = checked_file(original, files).decode("utf-8")
        if kind == "aux-build":
            declared = re.search(r'#!\[crate_name\s*=\s*"([A-Za-z_][A-Za-z0-9_]*)"\]', text)
            if declared:
                name = declared[1]
        if (name, original) in seen:
            continue
        seen.add((name, original))
        edition = 2015
        nested = []
        for line in text.splitlines():
            directive = line.strip()
            match = re.fullmatch(r"//@\s*edition:\s*(2015|2018|2021|2024)", directive)
            if match:
                edition = int(match[1])
            match = re.fullmatch(r"//@\s*(aux-build|aux-crate)\s*:\s*(\S.*)", directive)
            if match:
                nested.append((match[1], match[2].strip()))
            if re.fullmatch(r"//@\s*proc-macro\s*:.*", directive):
                blockers.append("proc-macro auxiliary requires procedural macro execution")
        flags += ["--extern", "%s%s=%s" % (prefix, name, SOURCE / original),
                  "--extern-edition", "%s=%d" % (name, edition)]
        sub_flags, sub_blockers = extern_flags(case_path, nested, files, seen, root=False)
        flags += sub_flags
        blockers += sub_blockers
    return flags, blockers


def contract(case, data, files):
    directives = [line.strip() for line in data.decode("utf-8").splitlines()
                  if line.lstrip().startswith("//@")]
    if directives != case["directives"]:
        raise ValueError("original directive inventory changed")
    edition = 2015  # Pinned compiletest default; never silently use LRR's 2024 default.
    blockers = []
    if directives.count("//@ run-pass") != 1:
        blockers.append("requires a run-pass contract adapter")
    editions = []
    reference_rules = []
    aux = []
    for directive in directives:
        if directive == "//@ run-pass":
            continue
        auxiliary = re.fullmatch(r"//@\s*(aux-build|aux-crate)\s*:\s*(\S.*)", directive)
        if auxiliary:
            aux.append((auxiliary[1], auxiliary[2].strip()))
            continue
        # Pinned compiletest recognizes `reference` but has no runtime handler
        # for it (directives/directive_names.rs and directives/handlers.rs).
        # Retain the Rust Reference rule ID as metadata; it is not an input file.
        reference = re.fullmatch(r"//@\s*reference:\s*([A-Za-z][A-Za-z0-9_.-]{0,255})", directive)
        if reference:
            reference_rules.append(reference[1])
            continue
        match = re.fullmatch(r"//@\s*edition:\s*(2015|2018|2021|2024)", directive)
        if match:
            editions.append(int(match[1]))
        else:
            blockers.append("unhandled directive: " + directive)
    if len(editions) > 1:
        blockers.append("multiple edition directives require configuration expansion")
    elif editions:
        edition = editions[0]
    externs, aux_blockers = extern_flags(case["path"], aux, files)
    blockers.extend(aux_blockers)
    stem = str(PurePosixPath(case["path"]).with_suffix(""))
    expected = [name for name in files if name.startswith(stem + ".")
                and name != case["path"]]
    if expected:
        blockers.append("expected-output/fixed files need their asserted-contract adapter")
    return {"mode": "run-pass", "edition": edition, "directives": directives,
            "expected_files": expected, "reference_rules": reference_rules,
            "extern_flags": externs, "blockers": blockers}


def observe(argv, directory, timeout):
    stdout = directory / "transport.stdout"
    stderr = directory / "host.stderr"
    start = time.monotonic()
    result = {"argv": argv, "timed_out": False, "output_limited": False,
              "exit_code": None}
    with stdout.open("xb") as out, stderr.open("xb") as err:
        try:
            child = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE)
        except OSError as error:
            result["launch_error"] = str(error)
        else:
            result["pid"] = child.pid
            # Bound both retained output and wall time while the child runs.
            # Direct file redirection would allow an overflowing child to fill
            # the disk before its output could be checked.
            sizes = {"stdout": 0, "stderr": 0}
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(child.stdout, selectors.EVENT_READ, ("stdout", out))
                    selector.register(child.stderr, selectors.EVENT_READ, ("stderr", err))
                    while selector.get_map():
                        remaining = timeout - (time.monotonic() - start)
                        if remaining <= 0:
                            result["timed_out"] = True
                            break
                        for key, _ in selector.select(min(remaining, 0.2)):
                            label, destination = key.data
                            data = os.read(key.fd, min(65_536, MAX_OUTPUT + 1 - sizes[label]))
                            if not data:
                                selector.unregister(key.fileobj)
                                continue
                            destination.write(data)
                            sizes[label] += len(data)
                            if sizes[label] > MAX_OUTPUT:
                                result["output_limited"] = True
                                break
                        if result["output_limited"]:
                            break
                    if not result["timed_out"] and not result["output_limited"]:
                        try:
                            child.wait(timeout=max(0, timeout - (time.monotonic() - start)))
                        except subprocess.TimeoutExpired:
                            result["timed_out"] = True
            finally:
                if child.poll() is None:
                    child.kill()
                result["exit_code"] = child.wait()
                child.stdout.close()
                child.stderr.close()
    result["elapsed_seconds"] = time.monotonic() - start
    for label, path in [("stdout", stdout), ("stderr", stderr)]:
        size = path.stat().st_size
        result[label + "_bytes"] = size
        if size <= MAX_OUTPUT:
            result[label + "_sha256"] = sha(path.read_bytes())
    return result


def integer(value):
    return type(value) is int and 0 <= value < 2**64


def classify(observation, packet, selected, context, files, digests):
    """Only a complete live capture can supply a selected runtime-contract pass."""
    if observation.get("launch_error"):
        return {"status": "host_launch_error"}
    if observation["timed_out"]:
        return {"status": "execution_timeout"}
    if observation["output_limited"] or max(
            observation["stdout_bytes"], observation["stderr_bytes"]) > MAX_OUTPUT:
        return {"status": "capture_size_limit"}
    if not isinstance(packet, dict):
        return {"status": "invalid_capture_protocol"}
    if (packet.get("schema") != "lrr/v5/standalone-runtime-capture/v1"
            or packet.get("request_context") != context
            or type(packet.get("process_id")) is not int
            or packet["process_id"] != observation.get("pid")):
        return {"status": "capture_identity_mismatch"}
    terminal = packet.get("terminal", {})
    if not isinstance(terminal, dict):
        return {"status": "invalid_capture_protocol"}
    if terminal.get("status") in ("fault", "suspended"):
        return {"status": "interpreter_failure", "terminal": terminal}
    if terminal.get("status") == "guest_panic":
        return {"status": "unexpected_guest_panic", "terminal": terminal}
    try:
        snapshot = packet["snapshot"]
        capture = packet["capture"]
        facts = capture["facts"]
        streams = capture["guest"]
        checks = {
            "clean_process_exit": observation["exit_code"] == 0
            and observation["stderr_bytes"] == 0,
            "protocol_not_guest_output": packet["transport_stdout_role"]
            == "protocol_json_not_guest_stdout",
            "separate_host_preparation": packet["host_preparation_streams"]
            == "outside_guest_capture_no_macro_host_or_build_output_promotion",
            "owning_edition": type(snapshot["owning_edition"]) is int
            and snapshot["owning_edition"] == selected["edition"]
            and type(terminal["live_edition"]) is int
            and terminal["live_edition"] == selected["edition"],
            "completed_main": terminal["schema"] == "lrr/v2/development-run"
            and terminal["semantics_profile"] == "lrr-2024-v2"
            and terminal["status"] == "completed" and terminal["value"] == {"kind": "unit"},
            "complete_capture": capture["complete"] is True and capture["blocker"] is None
            and capture["scope"] == "dispatcher_external_effects_only_not_memory_trace",
            "closed_guest_streams": streams == {
                "status": "closed_no_external_effects_observed", "stdout": [], "stderr": []},
            "actual_dispatch": integer(facts["vm_dispatches"])
            and facts["vm_dispatches"] > 0
            and integer(facts["vm_outcomes"])
            and facts["vm_outcomes"] == facts["vm_dispatches"],
            "terminal_and_cleanup": facts["terminal"] == {"kind": "completed", "unit": True}
            and facts["terminal"]["unit"] is True
            and facts["cleanup_observed"] is True and facts["closed_no_external_effects"] is True,
            "no_unobserved_boundaries": facts["observed_fault"] is False
            and facts["observed_suspension"] is False
            and all(type(facts[key]) is int and facts[key] == 0 for key in (
                "effect_recipe_dispatch_attempts", "unsupported_operation_candidates",
                "typed_fault_exit_candidates")),
            "root_path": snapshot["root_source"] == PurePosixPath(selected["case"]).name,
            "entry_identity": isinstance(packet["entry_body"], dict)
            and all(isinstance(packet["entry_body"].get(key), str)
                    and HEX.fullmatch(packet["entry_body"][key]) is not None
                    for key in ("definition", "exact_body_bytes")),
        }
        observed_files = snapshot["files"]
        if not isinstance(observed_files, list) or not 1 <= len(observed_files) <= 10_000:
            raise ValueError("capture source inventory bound")
        paths = []
        for entry in observed_files:
            relative = entry["path"]
            original = str(PurePosixPath(selected["case"]).parent / relative)
            data = checked_file(original, files)
            if (type(entry["bytes"]) is not int or entry["bytes"] != len(data)
                    or not isinstance(entry["content"], str)
                    or entry["content"] != digests[original]["content"]):
                raise ValueError("captured source metadata differs")
            paths.append(relative)
        checks["captured_originals"] = (bool(paths) and len(paths) == len(set(paths))
                                        and snapshot["root_source"] in paths)
    except (KeyError, TypeError, ValueError, OSError) as error:
        return {"status": "incomplete_evidence", "reason": str(error)}
    return {"status": "runtime_contract_pass" if all(checks.values()) else "incomplete_evidence",
            "checks": checks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lrr", type=Path, required=True)
    parser.add_argument("--source-digester", type=Path, required=True,
                        help="Prebuilt xtask lrr_source_digests example; no build is run here")
    parser.add_argument("--report-dir", type=Path, required=True)
    parser.add_argument("--case", action="append", required=True, dest="cases")
    parser.add_argument("--timeout", type=int, default=60)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("this adapter currently records macOS ARM64 evidence only")
    if not 1 <= args.timeout <= 600 or not 1 <= len(args.cases) <= 64:
        parser.error("timeout or selected-case bound exceeded")
    if len(args.cases) != len(set(args.cases)):
        parser.error("duplicate selected case")
    binary = args.lrr.resolve(strict=True)
    digester = args.source_digester.resolve(strict=True)
    directory = args.report_dir.resolve()
    if directory.exists() or directory.is_relative_to(ROOT):
        parser.error("report directory must be new and outside this frozen source")
    manifest_bytes = (SOURCE / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest["commit"] != COMMIT or manifest["license"] != "MIT OR Apache-2.0":
        parser.error("unreviewed upstream revision/license")
    files = {row["path"]: row for row in manifest["files"]}
    cases = {row["path"]: row for row in manifest["cases"]}
    if len(files) != len(manifest["files"]) or len(cases) != len(manifest["cases"]):
        parser.error("duplicate upstream inventory path")
    if any(name not in cases for name in args.cases):
        parser.error("selected path absent from the case inventory")
    for name in files:
        checked_file(name, files)
    if not {"LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"}.issubset(files):
        parser.error("missing original license inputs")
    binary_hash = sha(binary.read_bytes())
    digester_hash = sha(digester.read_bytes())
    directory.mkdir(parents=True)
    report = {"schema": "lrr/v5/selected-upstream-runtime/v1", "upstream_commit": COMMIT,
              "platform": "aarch64-apple-darwin", "manifest_sha256": sha(manifest_bytes),
              "interpreter_binary": str(binary), "interpreter_sha256": binary_hash,
              "source_digester": str(digester), "source_digester_sha256": digester_hash,
              "native_execution": False,
              "selected_cases": [{"case": name, "status": "unexecuted"} for name in args.cases],
              "complete_inventory": False,
              "upstream_runtime_acceptance": {"status": "incomplete", "counts": {}},
              "diagnostic_compatibility": {"status": "unassessed"},
              "compiler_tooling_compatibility": {"status": "unassessed"},
              "full_suite_compatibility": {"status": "incomplete"}, "guest_jit_allowed": False}
    try:
        digest_dir = directory / "source-digests"
        digest_dir.mkdir()
        receipt = observe([str(digester), str(SOURCE)], digest_dir, args.timeout)
        report["source_digest_observation"] = receipt
        if (receipt.get("launch_error") or receipt["timed_out"] or receipt["output_limited"]
                or receipt["exit_code"] != 0 or receipt["stderr_bytes"] != 0):
            raise ValueError("source digest helper did not complete")
        index = json.loads((digest_dir / "transport.stdout").read_bytes())
        if index["schema"] != "lrr/v5/source-digest-index/v1":
            raise ValueError("source digest protocol mismatch")
        digests = index["files"]
        if not isinstance(digests, dict) or set(digests) != set(files):
            raise ValueError("source digest inventory differs")
        for name, row in digests.items():
            if (type(row["bytes"]) is not int or row["bytes"] != files[name]["bytes"]
                    or not isinstance(row["content"], str) or HEX.fullmatch(row["content"]) is None):
                raise ValueError("invalid source digest row")
        for index, name in enumerate(args.cases):
            data = checked_file(name, files)
            selected = report["selected_cases"][index]
            selected.update(contract(cases[name], data, files), source_sha256=sha(data))
            if selected["blockers"]:
                selected["status"] = "unsupported_harness"
                continue
            case_dir = directory / str(index)
            case_dir.mkdir()
            context = secrets.token_hex(32)
            selected["capture_context"] = context
            observation = observe([
                str(binary), "--edition", str(selected["edition"])] + selected["extern_flags"] + [
                "run", str(SOURCE / name),
                "--runtime-capture", "--capture-context", context], case_dir, args.timeout)
            selected["observation"] = observation
            packet = None
            if observation["stdout_bytes"] <= MAX_OUTPUT:
                try:
                    packet = json.loads((case_dir / "transport.stdout").read_bytes())
                except (ValueError, UnicodeError):
                    pass
            selected.update(classify(observation, packet, selected, context, files, digests))
            checked_file(name, files)
            if (SOURCE / "manifest.json").read_bytes() != manifest_bytes:
                raise ValueError("manifest changed during execution")
            if sha(binary.read_bytes()) != binary_hash:
                raise ValueError("interpreter binary changed during execution")
        # Include blocked cases, licenses and auxiliary originals in the final
        # guard, even when no selected case reached execution.
        for name in files:
            checked_file(name, files)
        if (SOURCE / "manifest.json").read_bytes() != manifest_bytes:
            raise ValueError("manifest changed during batch")
        if sha(binary.read_bytes()) != binary_hash:
            raise ValueError("interpreter binary changed during batch")
        if sha(digester.read_bytes()) != digester_hash:
            raise ValueError("source digest helper changed during batch")
        report["input_guard"] = "all_imported_originals_manifest_and_binary_unchanged"
    except (KeyError, TypeError, ValueError, OSError) as error:
        report["evidence_error"] = str(error)
        for row in report["selected_cases"]:
            if row["status"] == "runtime_contract_pass":
                row["status"] = "invalidated_evidence"
    finally:
        report["upstream_runtime_acceptance"]["counts"] = dict(Counter(
            row["status"] for row in report["selected_cases"]))
        report["selected_batch_passed"] = (
            "evidence_error" not in report and len(report["selected_cases"]) == len(args.cases)
            and all(row["status"] == "runtime_contract_pass" for row in report["selected_cases"]))
        (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(directory / "report.json")
    return 0 if report["selected_batch_passed"] else 1


if __name__ == "__main__":
    sys.exit(main())

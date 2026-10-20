"""LRR initializer/exact-once control, NOT an unchanged upstream fixture pass.

Retains pinned repr128.rs declarations verbatim; only replaces its main outside
the licensed originals. Runtime interpretation/expansion is always the real CLI.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from rustc_checkout_runtime import observe


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--lrr", type=Path, default=ROOT / "target/debug/lrr")
    parser.add_argument("--editions", type=int, nargs="+", default=[2015, 2018, 2021, 2024])
    parser.add_argument("--origins", nargs="+", default=["ordinary", "emit", "wrapper", "standard"])
    parser.add_argument("--repeats", type=int, nargs="+", default=[1, 2])
    args = parser.parse_args()
    directory = args.directory.resolve()
    directory.mkdir(exist_ok=False)
    binary = args.lrr.resolve(strict=True)
    original = ROOT / "fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/repr128.rs"
    data = original.read_bytes()
    prefix = data.decode().split("fn main()", 1)[0]
    rows = []
    report = {
        "scope": "LRR initializer/exact-once adaptation, not a stock fixture pass",
        "upstream_original": str(original.relative_to(ROOT)),
        "original_sha256": hashlib.sha256(data).hexdigest(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "rows": rows,
    }
    macros = "macro_rules! emit {($e:expr)=>{$e}} macro_rules! wrap {($e:expr)=>{emit!($e)}}\n"
    cases = [
        ("i128", "Signed::U64Limit", "18446744073709551616i128"),
        ("i128", "Signed::BigNegative", "-170141183460469231731687303715884105728i128"),
        ("i128", "Signed::Next", "-170141183460469231731687303715884105727i128"),
        ("u128", "Unsigned::U64Limit", "18446744073709551616u128"),
        ("u128", "Unsigned::Next", "18446744073709551617u128"),
    ]
    for edition in args.editions:
        for origin in args.origins:
            for repeat in args.repeats:
                destination = directory / f"{edition}-{origin}-{repeat}"
                destination.mkdir()
                statements = []
                for _ in range(repeat):
                    for ty, variant, expected in cases:
                        expression = f"discriminant_value(&{variant})"
                        if origin == "emit":
                            expression = f"emit!({expression})"
                        elif origin == "wrapper":
                            expression = f"wrap!({expression})"
                        elif origin not in ["ordinary", "standard"]:
                            raise ValueError("unknown control origin")
                        statements.append(
                            f"assert_eq!({expression}, {expected});" if origin == "standard" else
                            f"let value: {ty} = {expression}; assert_eq!(value, {expected});")
                source = prefix + macros + "fn main() {\n" + "\n".join(statements) + "\n}\n"
                path = destination / "main.rs"
                path.write_text(source)
                context = secrets.token_hex(32)
                argv = [str(binary), "--edition", str(edition), "run", str(path),
                        "--runtime-capture", "--capture-context", context]
                observation = observe(argv, destination, 240)
                try:
                    packet = json.loads((destination / "transport.stdout").read_text())
                except (ValueError, OSError):
                    packet = {}
                capture = packet.get("capture", {})
                facts = capture.get("facts", {})
                checks = {
                    "process": observation.get("exit_code") == 0 and observation.get("stderr_bytes") == 0,
                    "identity": packet.get("request_context") == context and packet.get("process_id") == observation.get("pid"),
                    "edition": packet.get("snapshot", {}).get("owning_edition") == edition,
                    "completed": packet.get("terminal", {}).get("status") == "completed" and
                        facts.get("terminal") == {"kind": "completed", "unit": True},
                    "capture": capture.get("complete") is True and capture.get("blocker") is None,
                    "cleanup": facts.get("cleanup_observed") is True and facts.get("closed_no_external_effects") is True,
                    "dispatch": facts.get("vm_dispatches", 0) > 0 and facts.get("vm_dispatches") == facts.get("vm_outcomes"),
                    "initializers": facts.get("initializer_frames_entered", 0) >= 3,
                    "empty_guest_streams": capture.get("guest", {}).get("stdout") == [] and capture.get("guest", {}).get("stderr") == [],
                    "no_fault_or_suspension": facts.get("observed_fault") is False and facts.get("observed_suspension") is False,
                }
                row = {"edition": edition, "origin": origin, "repeat": repeat,
                       "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
                       "observation": observation, "checks": checks, "facts": facts,
                       "terminal": {k: v for k, v in packet.get("terminal", {}).items()
                                    if k in ["status", "code", "message"]},
                       "pass": all(checks.values())}
                rows.append(row)
                (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
                print(json.dumps({k: row[k] for k in ["edition", "origin", "repeat", "pass", "terminal"]}), flush=True)
    exact_once = []
    for edition in args.editions:
        for origin in args.origins:
            group = [r for r in rows if r["edition"] == edition and r["origin"] == origin]
            counts = [r["facts"].get("initializer_frames_entered") for r in group]
            exact_once.append({"edition": edition, "origin": origin, "counts": counts,
                               "pass": all(r["pass"] for r in group) and len(set(counts)) == 1})
    report["exact_once"] = exact_once
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return 0 if all(r["pass"] for r in rows) and all(r["pass"] for r in exact_once) else 1


if __name__ == "__main__":
    os.environ["LRR_EMBEDDED_FACADE"] = "1"
    raise SystemExit(main())

"""Run exact host controls from a tracked selection against a fingerprinted build."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import time

from source_manifest import ROOT, capture, sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("selection", type=Path)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    directory = args.directory.resolve()
    built = json.loads((directory / "after-build.json").read_text())
    binary = Path(built["binaries"]["test"]["path"])
    expected = built["binaries"]["test"]["sha256"]
    selectors = json.loads(args.selection.read_text())["controls"]
    if not selectors or len(set(selectors)) != len(selectors):
        parser.error("selection must contain distinct exact controls")
    if not built["source_matches_before"] or not built["head_matches_before"]:
        parser.error("build input guard failed")
    if sha(binary) != expected:
        parser.error("test binary changed since the build")
    listing = subprocess.check_output([str(binary), "--list"], cwd=ROOT, text=True)
    for selector in selectors:
        if listing.splitlines().count(selector + ": test") != 1:
            parser.error("control does not select exactly one test: " + selector)
    output = directory / "control-results.json"
    if output.exists() and not args.dry_run:
        parser.error("results already exist; use a new run directory")
    rows = []
    for index, selector in enumerate(selectors):
        current = capture()
        if (current["files"] != built["files"] or current["head"] != built["head"]
                or sha(binary) != expected):
            parser.error("source, HEAD or test binary changed since the build")
        argv = [str(binary), selector, "--exact", "--nocapture", "--test-threads=1"]
        if args.dry_run:
            print(json.dumps({"argv": argv, "executed": False}))
            continue
        started = time.monotonic()
        log = directory / ("control-%d.log" % index)
        with log.open("xb") as stream:
            result = subprocess.run(argv, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
        current = capture()
        unchanged = (current["files"] == built["files"]
                     and current["head"] == built["head"] and sha(binary) == expected)
        one = len(re.findall(r"^running 1 test$", log.read_text(), re.MULTILINE)) == 1
        row = {"selector": selector, "argv": argv, "exit": result.returncode,
               "one_body": one, "inputs_unchanged": unchanged,
               "pass": result.returncode == 0 and one and unchanged,
               "seconds": round(time.monotonic() - started, 3), "log": log.name}
        rows.append(row)
        output.write_text(json.dumps(rows, indent=2) + "\n")
        print(json.dumps(row), flush=True)
        if not unchanged:
            return 1
    return 0 if args.dry_run or all(row["pass"] for row in rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())

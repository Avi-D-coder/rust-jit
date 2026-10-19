"""Fingerprint validation inputs and binaries; outputs are disposable evidence.

No compiler or test is started here. Cargo's JSON output selects the actual lib
test executable, avoiding a dependency on a previous build's hashed filename.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
INPUTS = ["crates", "vendor", "xtask", "tools", "fixtures", "compat", "src", ".cargo",
          "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]


def sha(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def capture():
    names = subprocess.check_output(
        ["git", "ls-files", "-co", "--exclude-standard", "-z", "--", *INPUTS],
        cwd=ROOT).split(b"\0")
    files = {os.fsdecode(name): sha(ROOT / os.fsdecode(name))
             for name in sorted(set(names)) if name and (ROOT / os.fsdecode(name)).is_file()}
    return {"schema": "lrr/validation-inputs/v2",
            "time_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                            text=True).strip(),
            "file_count": len(files), "files": files,
            "source_sha256": hashlib.sha256(json.dumps(
                files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("phase", choices=("before", "after-build", "after-runtime"))
    parser.add_argument("--cargo-messages", type=Path)
    parser.add_argument("--cli-binary", type=Path)
    args = parser.parse_args()
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    output = directory / (args.phase + ".json")
    if output.exists():
        parser.error("phase evidence already exists; use a new run directory")
    record = capture()
    binaries = {}
    if args.cargo_messages:
        candidates = set()
        for line in args.cargo_messages.read_text().splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if (row.get("reason") == "compiler-artifact"
                    and row.get("target", {}).get("name") == "lrr"
                    and "lib" in row.get("target", {}).get("kind", [])
                    and row.get("profile", {}).get("test") is True
                    and row.get("executable")):
                candidates.add(row["executable"])
        if len(candidates) != 1:
            parser.error("Cargo messages must identify exactly one lrr library test executable")
        binaries["test"] = Path(candidates.pop()).resolve(strict=True)
    if args.cli_binary:
        binaries["cli"] = args.cli_binary.resolve(strict=True)
    record["binaries"] = {name: {"path": str(path), "sha256": sha(path)}
                          for name, path in binaries.items()}
    valid = True
    if args.phase != "before":
        before = json.loads((directory / "before.json").read_text())
        record["source_matches_before"] = before["files"] == record["files"]
        record["head_matches_before"] = before["head"] == record["head"]
        record["changed_sources"] = sorted(
            name for name in set(before["files"]) | set(record["files"])
            if before["files"].get(name) != record["files"].get(name))
        valid = record["source_matches_before"] and record["head_matches_before"]
    if args.phase == "after-runtime":
        built = json.loads((directory / "after-build.json").read_text())
        current = {name: {"path": row["path"], "sha256": sha(Path(row["path"]))}
                   for name, row in built["binaries"].items()}
        record["binaries_match_build"] = bool(current) and current == built["binaries"]
        record["binaries"] = current
        valid = valid and record["binaries_match_build"]
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({key: value for key, value in record.items() if key != "files"}))
    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())

"""Copy unchanged rustc UI cases, helpers, snapshots and licenses from a local checkout."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "fixtures/upstream/rustc-checkout"


def git(checkout, *args):
    return subprocess.check_output(["git", "-C", str(checkout), *args], text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkout", type=Path, default=ROOT / "rustc/rust")
    parser.add_argument("--support-file", action="append", default=[],
                        help="Copy an explicit original input without registering a test case")
    parser.add_argument("cases", nargs="+")
    args = parser.parse_args()
    checkout = args.checkout.resolve()
    commit = git(checkout, "rev-parse", "HEAD")
    assert 1 <= len(args.cases) <= 64
    assert len(args.support_file) <= 256
    # compiletest retains root_testpaths for every auxiliary of a test, even
    # when that auxiliary declares another helper (runtest.rs::build_auxiliary).
    pending = [(name, PurePosixPath(name).parent / "auxiliary") for name in args.cases]
    pending.extend((name, None) for name in args.support_file)
    pending.extend((name, None) for name in ["LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"])
    files = {}
    cases = {}
    total = 0
    while pending:
        name, auxiliary_root = pending.pop(0)
        if name in files:
            continue
        path = PurePosixPath(name)
        assert not path.is_absolute() and ".." not in path.parts
        source = (checkout / name).resolve()
        source.relative_to(checkout)
        assert source.is_file() and source.stat().st_size <= 1024 * 1024
        data = source.read_bytes()
        total += len(data)
        assert total <= 16 * 1024 * 1024
        sha = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        assert sha == git(checkout, "rev-parse", f"{commit}:{name}"), f"changed checkout file: {name}"
        files[name] = {"path": name, "git_blob": sha, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
        if name.endswith(".rs"):
            text = data.decode("utf-8")
            directives = [line.strip() for line in text.splitlines() if line.lstrip().startswith("//@")]
            if name in args.cases:
                cases[name] = {"path": name, "directives": directives, "lrr_acceptance": "pending", "native_acceptance": "unexecuted"}
            for match in re.finditer(r"^\s*//(?:@)?\s*(?:proc-macro|aux-build):\s*(\S+)", text, re.MULTILINE):
                auxiliary = PurePosixPath(match.group(1))
                assert not auxiliary.is_absolute() and ".." not in auxiliary.parts
                assert auxiliary_root is not None
                pending.append((str(auxiliary_root / auxiliary), auxiliary_root))
            pending.extend((str(p.relative_to(checkout)), auxiliary_root) for p in sorted(source.parent.glob(source.stem + ".*")) if p.is_file() and p.suffix in (".stdout", ".stderr", ".fixed"))
    manifest = {
        "schema_version": 1, "repository": "https://github.com/rust-lang/rust", "commit": commit,
        "license": "MIT OR Apache-2.0", "scope": "Selected unchanged official UI cases; no complete-suite or execution claim",
        "files": [files[name] for name in sorted(files)], "cases": [cases[name] for name in sorted(cases)],
    }
    # Re-importing must not silently overwrite evidence from another upstream revision.
    manifest_path = DESTINATION / "manifest.json"
    if manifest_path.exists():
        old = json.loads(manifest_path.read_text())
        assert old["commit"] == commit, "choose a distinct import folder for a different revision"
        for kind in ["files", "cases"]:
            entries = {entry["path"]: entry for entry in old[kind]}
            for entry in manifest[kind]:
                if entry["path"] in entries:
                    assert entries[entry["path"]] == entry, "existing import metadata differs"
                entries[entry["path"]] = entry
            manifest[kind] = [entries[name] for name in sorted(entries)]
    for name in sorted(files):
        target = DESTINATION / name
        data = (checkout / name).read_bytes()
        assert hashlib.sha256(data).hexdigest() == files[name]["sha256"]
        if target.exists():
            assert target.read_bytes() == data, "existing copied test was modified"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Copied {len(cases)} cases and verified {len(files)} files at {commit}; no execution claimed")


if __name__ == "__main__":
    main()

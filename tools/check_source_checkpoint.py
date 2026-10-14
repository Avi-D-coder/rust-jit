"""Reject build outputs and oversized blobs in a staged source checkpoint."""
import subprocess
import sys
import os
import hashlib
import json
from pathlib import PurePosixPath


def local_output(path):
    parts = path.split(b"/")
    directories = parts[:-1]
    if parts[0] in (b"artifacts", b"rustc", b"coverage", b"logs"):
        return True
    if any(part in (
        b"target", b".lrr", b"__pycache__", b".pytest_cache", b".mypy_cache",
        b".ruff_cache", b".tox", b".nox", b".venv", b"venv", b"htmlcov",
        b".idea", b".vscode", b".AppleDouble",
    ) for part in directories):
        return True
    # Do not apply extension rules to pinned inputs, which may be test witnesses.
    pinned = path.startswith((b"vendor/", b"fixtures/upstream/rustc-checkout/")) or b"/fixtures/upstream/rustc-checkout/" in path
    if pinned:
        return False
    name = parts[-1]
    if name in (b".DS_Store", b".LSOverride", b".coverage") or name.startswith((b"._", b".coverage.")):
        return True
    if name == b".env" or name.startswith(b".env."):
        return name not in (b".env.example", b".env.template")
    if name.endswith((b".pyc", b".pyo", b".pyd", b".swp", b".swo", b"~")):
        return True
    if parts[0] in (b"crates", b"xtask") and name.endswith((b".orig", b".rej")):
        return True
    if len(parts) == 1 and name.endswith((b".profraw", b".profdata")):
        return True
    return bool(directories and directories[0].endswith(b".dSYM"))


def git(*args, input=None):
    return subprocess.check_output(["git", *args], input=input)


def fixture_root(path):
    marker = b"fixtures/upstream/rustc-checkout/"
    if path.startswith(marker):
        return marker[:-1]
    head, separator, _ = path.partition(b"/" + marker)
    return head + b"/" + marker[:-1] if separator else None


def fixture_manifest_errors(changed, index):
    """Check immutable originals using only staged files and staged metadata."""
    roots = {root for path in changed if (root := fixture_root(path)) is not None}
    errors = []
    for root in sorted(roots):
        prefix = root + b"/"
        manifest = prefix + b"manifest.json"
        label = os.fsdecode(manifest)
        members = {path[len(prefix):]: entry for path, entry in index.items()
                   if path.startswith(prefix)}
        if manifest not in index:
            if any(path not in (b"README.md",) for path in members):
                errors.append(f"upstream originals lack an indexed manifest: {label}")
            continue  # Removing a complete old input directory is permitted.
        mode, blob = index[manifest]
        if mode not in ("100644", "100755") or int(git("cat-file", "-s", blob)) > 5 * 1024 * 1024:
            errors.append(f"invalid upstream manifest file: {label}")
            continue
        try:
            metadata = json.loads(git("cat-file", "blob", blob))
            files = metadata["files"]
            if not isinstance(files, list):
                raise ValueError("files must be a list")
            expected = {}
            for entry in files:
                name = entry["path"]
                path = PurePosixPath(name)
                if (not isinstance(name, str) or not name or path.is_absolute()
                        or any(part in ("", ".", "..") for part in name.split("/"))
                        or "\0" in name):
                    raise ValueError("invalid original path")
                encoded = name.encode("utf-8")
                if encoded in expected:
                    raise ValueError("duplicate original path")
                expected[encoded] = entry
            for name, entry in expected.items():
                display = os.fsdecode(prefix + name)
                staged = members.get(name)
                if staged is None:
                    errors.append(f"upstream original missing from index: {display}")
                    continue
                mode, blob = staged
                if mode not in ("100644", "100755"):
                    errors.append(f"upstream original is not a regular file: {display}")
                    continue
                size = int(git("cat-file", "-s", blob))
                if size > 5 * 1024 * 1024:
                    errors.append(f"upstream original exceeds 5 MiB: {display}")
                    continue
                raw = git("cat-file", "blob", blob)
                if len(raw) != entry["bytes"] or hashlib.sha256(raw).hexdigest() != entry["sha256"]:
                    errors.append(f"upstream original differs from indexed import manifest: {display}")
                if "git_blob" in entry:
                    header = b"blob " + str(len(raw)).encode("ascii") + b"\0"
                    if hashlib.sha1(header + raw).hexdigest() != entry["git_blob"]:
                        errors.append(f"upstream original Git blob differs from import manifest: {display}")
            for name in sorted(members.keys() - expected.keys() - {b"manifest.json", b"README.md"}):
                errors.append(f"upstream file absent from indexed import manifest: {os.fsdecode(prefix + name)}")
        except (KeyError, TypeError, ValueError, UnicodeError) as error:
            errors.append(f"invalid upstream import manifest {label}: {error}")
    return errors


def main():
    paths = set(git("diff", "--cached", "--no-renames", "--name-only", "--diff-filter=ACMR", "-z").split(b"\0"))
    paths.discard(b"")
    index = {}
    for row in git("ls-files", "--stage", "-z").split(b"\0"):
        if row:
            metadata, path = row.split(b"\t", 1)
            mode, blob, stage = metadata.split()
            if stage == b"0":
                index[path] = (mode.decode("ascii"), blob.decode("ascii"))
    entries = {path: entry for path, entry in index.items() if path in paths}
    changed = set(git("diff", "--cached", "--no-renames", "--name-only", "--diff-filter=ACMRD", "-z").split(b"\0"))
    changed.discard(b"")
    errors = fixture_manifest_errors(changed, index)
    total = 0
    for path, (mode, blob) in sorted(entries.items()):
        name = path.decode("utf-8", errors="backslashreplace")
        size = int(git("cat-file", "-s", blob))
        total += size
        if local_output(path):
            errors.append(f"local build/validation input staged: {name}")
        if size > 5 * 1024 * 1024:
            errors.append(f"blob exceeds 5 MiB: {name} ({size} bytes)")
        if path.endswith(b".patch") or path.startswith((b"vendor/", b"fixtures/upstream/rustc-checkout/")) or b"/fixtures/upstream/rustc-checkout/" in path:
            if mode == "120000":
                raw = os.fsencode(os.readlink(os.fsdecode(path)))
            else:
                with open(path, "rb") as source:
                    raw = source.read()
            actual = git("hash-object", "--stdin", "--no-filters", input=raw).decode().strip()
            if actual != blob:
                errors.append(f"pinned input changed by Git filters: {name}")
    for error in errors:
        print(error, file=sys.stderr)
    print(f"Source checkpoint: {len(entries)} staged files, {total} bytes, {len(errors)} violations.")
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())

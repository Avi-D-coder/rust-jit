"""Reject build outputs and oversized blobs in a staged source checkpoint."""
import subprocess
import sys
import os


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


def main():
    paths = set(git("diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z").split(b"\0"))
    paths.discard(b"")
    entries = {}
    for row in git("ls-files", "--stage", "-z").split(b"\0"):
        if row:
            metadata, path = row.split(b"\t", 1)
            mode, blob, stage = metadata.split()
            if stage == b"0" and path in paths:
                entries[path] = (mode.decode("ascii"), blob.decode("ascii"))
    errors = []
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

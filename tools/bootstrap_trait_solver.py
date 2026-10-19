#!/usr/bin/env python3
"""Import/audit pinned upstream solver sources; never run upstream build scripts.

Python3.11+. The default audit needs only Git-owned inputs. Import requires the
official checkout, reads exact Git objects and refuses to replace an existing tree.
Only Cargo packaging changes; all compiler Rust source bytes remain unchanged.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
TREE = ROOT / "vendor/rustc-trait-solver"
MANIFEST = ROOT / "vendor/rustc-trait-solver.files.json"
CRATES = (
    "rustc_next_trait_solver", "rustc_type_ir", "rustc_type_ir_macros",
    "rustc_index", "rustc_index_macros", "rustc_abi", "rustc_ast_ir", "rustc_hashes",
)
LICENSES = ("LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT")
SUPPORT_SOURCES = ("compiler/rustc_middle/src/ty/structural_impls.rs",)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git(checkout, *args):
    return subprocess.check_output(["git", "-C", str(checkout), *args])


def stable_manifest(original):
    """Stable shared-crate packaging; no changes to source/solver algorithms."""
    parsed = tomllib.loads(original.decode())

    def value(item):
        if isinstance(item, str):
            return json.dumps(item)
        if isinstance(item, bool):
            return "true" if item else "false"
        if isinstance(item, list):
            return "[" + ", ".join(value(x) for x in item) + "]"
        if isinstance(item, dict):
            return "{ " + ", ".join(k + " = " + value(v) for k, v in item.items()) + " }"
        if isinstance(item, int):
            return str(item)
        raise ValueError("unsupported manifest value")

    package = parsed["package"]
    if set(parsed) - {"package", "dependencies", "features", "lib"}:
        raise ValueError("unexpected upstream manifest sections")
    if set(package) != {"name", "version", "edition"}:
        raise ValueError("unexpected upstream package fields")
    lines = ["# Stable shared-library packaging; original: Cargo.toml.upstream", "[package]"]
    for key, item in package.items():
        lines.append(key + " = " + value(item))
    lines.extend(['license = "MIT OR Apache-2.0"', 'rust-version = "1.99"'])
    if "lib" in parsed:
        lines.extend(["", "[lib]"])
        lines.extend(k + " = " + value(v) for k, v in parsed["lib"].items())
    lines.extend(["", "[dependencies]"])
    for name, dependency in parsed.get("dependencies", {}).items():
        if isinstance(dependency, dict) and dependency.get("optional"):
            continue  # Compiler-internal/nightly/randomized-layout dependency.
        if isinstance(dependency, dict) and "path" in dependency:
            if dependency["path"] != "../" + name or name not in CRATES:
                raise ValueError("unadmitted stable path dependency: " + name)
        lines.append(name + " = " + value(dependency))
    # Preserve cfg feature names for check-cfg, but none enables compiler-only
    # dependencies. The supported integration always has default-features=false.
    lines.extend(["", "[features]", "default = []"])
    for name in parsed.get("features", {}):
        if name != "default":
            lines.append(name + " = []")
    return ("\n".join(lines) + "\n").encode()


def workspace_manifest():
    members = ["compiler/" + name for name in CRATES]
    return ("[workspace]\nmembers = " + json.dumps(members)
            + '\nresolver = "3"\n').encode()


def import_sources(checkout):
    if TREE.exists() or MANIFEST.exists():
        raise ValueError("source tree/inventory exists; audit it, never overwrite local work")
    git(checkout, "cat-file", "-e", COMMIT + "^{commit}")
    paths = git(checkout, "ls-tree", "-r", "--name-only", COMMIT, "--",
                *("compiler/" + name for name in CRATES), *LICENSES,
                *SUPPORT_SOURCES).decode().splitlines()
    originals = {}
    for path in paths:
        raw = git(checkout, "show", COMMIT + ":" + path)
        originals[path] = raw
    installed = dict(originals)
    for name in CRATES:
        path = "compiler/" + name + "/Cargo.toml"
        original = originals[path]
        installed[path + ".upstream"] = original
        installed[path] = stable_manifest(original)
    installed["Cargo.toml"] = workspace_manifest()
    rows = []
    for path, raw in sorted(installed.items()):
        destination = TREE / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
        upstream = path.removesuffix(".upstream")
        row = {"path": path, "bytes": len(raw), "sha256": digest(raw)}
        if upstream in originals:
            row.update(upstream=upstream, upstream_sha256=digest(originals[upstream]))
        rows.append(row)
    MANIFEST.write_text(json.dumps({
        "schema": 2, "repository": "https://github.com/rust-lang/rust.git",
        "commit": COMMIT, "crates": list(CRATES), "files": rows,
        "support_sources": list(SUPPORT_SOURCES),
        "packaging": "stable-shared-v1: unchanged Rust; retain upstream manifests; omit optional compiler-only dependencies; disable default/nightly feature activation",
        "supported_features": "default-features=false; no compiler-only/nightly/randomize features",
    }, indent=2) + "\n")


def audit(checkout=None):
    inventory = json.loads(MANIFEST.read_text())
    if (inventory["schema"] != 2 or inventory["commit"] != COMMIT
            or inventory["crates"] != list(CRATES)
            or inventory.get("support_sources") != list(SUPPORT_SOURCES)):
        raise ValueError("solver inventory pin/closure differs")
    expected = set()
    total = 0
    for row in inventory["files"]:
        path = row["path"]
        relative = Path(path)
        if relative.is_absolute() or ".." in relative.parts or path in expected:
            raise ValueError("invalid/duplicate inventory path: " + path)
        expected.add(path)
        target = TREE / path
        if target.is_symlink():
            raise ValueError("unexpected source symlink: " + path)
        raw = target.read_bytes()
        if len(raw) != row["bytes"] or digest(raw) != row["sha256"]:
            raise ValueError("installed source differs: " + path)
        total += len(raw)
        if path.endswith("/Cargo.toml"):
            original = target.with_name("Cargo.toml.upstream").read_bytes()
            if stable_manifest(original) != raw:
                raise ValueError("stable manifest projection differs: " + path)
        elif path == "Cargo.toml":
            if workspace_manifest() != raw:
                raise ValueError("solver workspace differs")
        elif row.get("upstream_sha256") != digest(raw):
            raise ValueError("upstream source bytes changed: " + path)
        if checkout and row.get("upstream"):
            original = git(checkout, "show", COMMIT + ":" + row["upstream"])
            if digest(original) != row["upstream_sha256"]:
                raise ValueError("upstream Git object differs: " + path)
    actual = {p.relative_to(TREE).as_posix() for p in TREE.rglob("*") if p.is_file()}
    # A dependency build may create its own lockfile; its exact resolution is
    # instead pinned by the top-level Cargo.lock used for the actual integration.
    actual.discard("Cargo.lock")
    if actual != expected or not set(LICENSES) <= expected:
        raise ValueError("solver source membership/licenses differ")
    return {"commit": COMMIT, "crates": len(CRATES), "files": len(expected),
            "source_bytes": total, "rust_source_changes": 0,
            "upstream_git_objects_verified": checkout is not None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkout", type=Path, default=ROOT / "rustc/rust")
    parser.add_argument("--import", action="store_true", dest="install")
    parser.add_argument("--verify-upstream", action="store_true")
    args = parser.parse_args()
    try:
        if args.install:
            import_sources(args.checkout)
        print(json.dumps(audit(args.checkout if args.install or args.verify_upstream else None)))
    except (OSError, KeyError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + "\n")


if __name__ == "__main__":
    main()

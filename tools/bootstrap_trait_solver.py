#!/usr/bin/env python3
"""Import/audit pinned upstream solver sources; never run upstream build scripts.

Python3.11+. The default audit needs only Git-owned inputs. Import requires the
official checkout, reads exact Git objects and refuses to replace an existing tree.
Retain all original Rust bytes. Registered mechanical stable build projections
reuse the pinned compiler helper instead of the shared library's panic stubs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
import upstream_solver_stable as stable

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
TREE = ROOT / "vendor/rustc-trait-solver"
MANIFEST = ROOT / "vendor/rustc-trait-solver.files.json"
CRATES = (
    "rustc_next_trait_solver", "rustc_type_ir", "rustc_type_ir_macros",
    "rustc_index", "rustc_index_macros", "rustc_abi", "rustc_ast_ir", "rustc_hashes",
)
LICENSES = ("LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT")
SUPPORT_SOURCES = (
    "compiler/rustc_middle/src/ty/structural_impls.rs",
    "compiler/rustc_middle/src/ty/fold.rs",
    "compiler/rustc_middle/src/ty/predicate.rs",
    'compiler/rustc_middle/src/ty/relate.rs',
    'compiler/rustc_middle/src/ty/consts/kind.rs',
    'compiler/rustc_middle/src/ty/abstract_const.rs',
    'compiler/rustc_middle/src/mir/syntax.rs',
    'compiler/rustc_middle/src/ty/erase_regions.rs',
    'compiler/rustc_middle/src/ty/trait_def.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/lib.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/mod.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/type_variable.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/unify_key.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/region_constraints/mod.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/snapshot/undo_log.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/opaque_types/mod.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/opaque_types/table.rs',
    'src/tools/rust-analyzer/crates/hir-ty/src/next_solver/infer/snapshot/mod.rs',
    'compiler/rustc_infer/src/infer/mod.rs',
    'compiler/rustc_infer/src/infer/type_variable.rs',
    'compiler/rustc_infer/src/infer/resolve.rs',
    'compiler/rustc_middle/src/ty/generic_args.rs',
    'compiler/rustc_infer/src/infer/solver_region_constraints.rs',
    'compiler/rustc_infer/src/infer/solver_region_constraints/tests.rs',
    'compiler/rustc_infer/src/infer/snapshot/undo_log.rs',
    'compiler/rustc_infer/src/infer/outlives/obligations.rs',
    'compiler/rustc_infer/src/infer/relate/higher_ranked.rs',
) + stable.SUPPORT_SOURCES


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


def packaged_sources(originals):
    installed = dict(originals)
    for name in CRATES:
        path = "compiler/" + name + "/Cargo.toml"
        original = originals[path]
        installed[path + ".upstream"] = original
        installed[path] = stable_manifest(original)
    installed["Cargo.toml"] = workspace_manifest()
    for path in stable.RETAIN_ORIGINAL:
        installed[path + ".upstream"] = originals[path]
    installed.update(stable.projections(originals))
    return installed


def source_inventory(originals, installed):
    rows = []
    for path, raw in sorted(installed.items()):
        upstream = stable.PROJECTION_SOURCES.get(path, path.removesuffix(".upstream"))
        row = {"path": path, "bytes": len(raw), "sha256": digest(raw)}
        if upstream in originals:
            row.update(upstream=upstream, upstream_sha256=digest(originals[upstream]))
        if path in stable.PROJECTION_SOURCES:
            row["mechanical_adapter"] = stable.ADAPTATION
        rows.append(row)
    return {
        "schema": 3, "repository": "https://github.com/rust-lang/rust.git",
        "commit": COMMIT, "crates": list(CRATES), "files": rows,
        "support_sources": list(SUPPORT_SOURCES),
        "packaging": "stable-shared-v2: retain all originals; registered stable-region-assumptions-v1 compiler-helper projections; retain upstream manifests; omit optional compiler-only dependencies; disable default/nightly feature activation",
        "rust_adaptation": stable.ADAPTATION,
        "supported_features": "default-features=false; no compiler-only/nightly/randomize features",
    }


def import_sources(checkout):
    if TREE.exists() or MANIFEST.exists():
        raise ValueError("source tree/inventory exists; audit it, never overwrite local work")
    git(checkout, "cat-file", "-e", COMMIT + "^{commit}")
    paths = git(checkout, "ls-tree", "-r", "--name-only", COMMIT, "--",
                *("compiler/" + name for name in CRATES), *LICENSES,
                *SUPPORT_SOURCES).decode().splitlines()
    originals = {path: git(checkout, "show", COMMIT + ":" + path) for path in paths}
    installed = packaged_sources(originals)
    for path, raw in installed.items():
        destination = TREE / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
    MANIFEST.write_text(json.dumps(source_inventory(originals, installed), indent=2) + "\n")


def audit(checkout=None):
    inventory = json.loads(MANIFEST.read_text())
    if (inventory["schema"] != 3 or inventory["commit"] != COMMIT
            or inventory["crates"] != list(CRATES)
            or inventory.get("support_sources") != list(SUPPORT_SOURCES)
            or inventory.get("rust_adaptation") != stable.ADAPTATION):
        raise ValueError("solver inventory pin/closure differs")
    originals = {path: (TREE / (path + ".upstream" if path in stable.RETAIN_ORIGINAL else path)).read_bytes()
                 for path in stable.HASHES}
    projections = stable.projections(originals)
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
        if path in projections:
            source = stable.PROJECTION_SOURCES[path]
            if (raw != projections[path]
                    or row.get("mechanical_adapter") != stable.ADAPTATION
                    or row.get("upstream") != source
                    or row.get("upstream_sha256") != digest(originals[source])):
                raise ValueError("registered stable Rust projection differs: " + path)
        elif "mechanical_adapter" in row:
            raise ValueError("unregistered stable Rust projection: " + path)
        elif path.endswith("/Cargo.toml"):
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
    if (actual != expected or not set(LICENSES) <= expected
            or not set(projections) <= expected
            or not {p + ".upstream" for p in stable.RETAIN_ORIGINAL} <= expected):
        raise ValueError("solver source membership/licenses differ")
    changed = sum(projections[path] != originals[source]
                  for path, source in stable.PROJECTION_SOURCES.items())
    return {"commit": COMMIT, "crates": len(CRATES), "files": len(expected),
            "source_bytes": total, "rust_source_changes": changed,
            "retained_original_rust_changes": 0,
            "registered_rust_adaptation": stable.ADAPTATION,
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

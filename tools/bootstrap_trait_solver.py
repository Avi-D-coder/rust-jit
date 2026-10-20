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
import upstream_transmute as transmute

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
TREE = ROOT / "vendor/rustc-trait-solver"
MANIFEST = ROOT / "vendor/rustc-trait-solver.files.json"
CRATES = (
    "rustc_next_trait_solver", "rustc_type_ir", "rustc_type_ir_macros",
    "rustc_index", "rustc_index_macros", "rustc_abi", "rustc_ast_ir", "rustc_hashes",
    "rustc_transmute",
)
LICENSES = ("LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT")
SUPPORT_SOURCES = (
    'compiler/rustc_hir_analysis/src/collect/clauses_of.rs',
    'compiler/rustc_hir_analysis/src/collect.rs',
    'compiler/rustc_hir_analysis/src/collect/item_bounds.rs',
    'compiler/rustc_hir_analysis/src/outlives/mod.rs',
    'compiler/rustc_hir_analysis/src/hir_ty_lowering/bounds.rs',
    'compiler/rustc_session/src/options.rs',
    'library/core/src/cmp.rs',
    'library/core/src/marker.rs',
    "compiler/rustc_hir_analysis/src/collect/type_of.rs",
    "compiler/rustc_middle/src/ty/generics.rs",
    "compiler/rustc_middle/src/ty/util.rs",
    "compiler/rustc_hir_analysis/src/variance/solve.rs",
    "compiler/rustc_hir_analysis/src/variance/constraints.rs",
    "compiler/rustc_hir_analysis/src/variance/terms.rs",
    "compiler/rustc_hir_analysis/src/variance/mod.rs",
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
    'compiler/rustc_middle/src/ty/mod.rs',
    'compiler/rustc_infer/src/infer/context.rs',
    'compiler/rustc_infer/src/infer/opaque_types/mod.rs',
    'compiler/rustc_infer/src/infer/opaque_types/table.rs',
    'compiler/rustc_infer/src/infer/solver_region_constraints.rs',
    'compiler/rustc_infer/src/infer/solver_region_constraints/tests.rs',
    'compiler/rustc_infer/src/infer/snapshot/undo_log.rs',
    'compiler/rustc_infer/src/infer/outlives/obligations.rs',
    'compiler/rustc_infer/src/infer/relate/higher_ranked.rs',
    'compiler/rustc_infer/src/infer/relate/generalize.rs',
    'compiler/rustc_infer/src/infer/canonical/mod.rs',
    'compiler/rustc_infer/src/infer/canonical/instantiate.rs',
    'compiler/rustc_infer/src/infer/canonical/query_response.rs',
    'compiler/rustc_infer/src/infer/outlives/mod.rs',
    'compiler/rustc_middle/src/infer/canonical.rs',
    'compiler/rustc_trait_selection/src/solve/delegate.rs',
    'compiler/rustc_trait_selection/src/traits/util.rs',
    'compiler/rustc_middle/src/ty/sty.rs',
    'compiler/rustc_infer/src/infer/region_constraints/mod.rs',
    'compiler/rustc_data_structures/src/graph/mod.rs',
    'compiler/rustc_data_structures/src/graph/reference.rs',
    'compiler/rustc_data_structures/src/graph/reversed.rs',
    'compiler/rustc_data_structures/src/graph/tests.rs',
    'compiler/rustc_data_structures/src/graph/iterate/mod.rs',
    'compiler/rustc_data_structures/src/graph/iterate/tests.rs',
    'compiler/rustc_data_structures/src/graph/scc/mod.rs',
    'compiler/rustc_data_structures/src/graph/scc/tests.rs',
    'compiler/rustc_data_structures/src/graph/vec_graph/mod.rs',
    'compiler/rustc_data_structures/src/graph/vec_graph/tests.rs',
) + stable.SUPPORT_SOURCES
SUPPORT_SOURCES = tuple(sorted(set(SUPPORT_SOURCES + ('compiler/rustc_data_structures/src/fx.rs', 'compiler/rustc_feature/src/unstable.rs', 'compiler/rustc_infer/src/infer/at.rs', 'compiler/rustc_middle/src/mir/interpret/queries.rs', 'compiler/rustc_middle/src/ty/adt.rs', 'compiler/rustc_middle/src/ty/consts.rs', 'compiler/rustc_middle/src/ty/consts/int.rs', 'compiler/rustc_middle/src/ty/consts/valtree.rs', 'compiler/rustc_middle/src/ty/context.rs', 'compiler/rustc_span/src/symbol.rs', 'compiler/rustc_trait_selection/src/traits/mod.rs', 'compiler/rustc_trait_selection/src/traits/project.rs', 'compiler/rustc_trait_selection/src/traits/wf.rs', 'compiler/rustc_ty_utils/src/ty.rs'))))
SUPPORT_SOURCES = tuple(sorted(set(SUPPORT_SOURCES + ('compiler/rustc_attr_ir/src/lang_items.rs',))))
TRANSMUTE_PREFIX = 'compiler/rustc_transmute/'
TRANSMUTE_ADAPTATION = 'stable-transmute-v1'


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
        installed[path] = transmute.CARGO.encode() if name == "rustc_transmute" else stable_manifest(original)
    installed["Cargo.toml"] = workspace_manifest()
    for path in stable.RETAIN_ORIGINAL:
        installed[path + ".upstream"] = originals[path]
    installed.update(stable.projections(originals))
    for name, expected_hash in transmute.PINS.items():
        path = TRANSMUTE_PREFIX + name
        raw = originals[path]
        if digest(raw) != expected_hash:
            raise ValueError("pinned transmute package source differs: " + path)
        installed[path + ".upstream"] = raw
        installed[path] = transmute.CARGO.encode() if name == "Cargo.toml" else transmute.stable(name, raw.decode())
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        installed[TRANSMUTE_PREFIX + name] = originals[name]
    return installed


def source_inventory(originals, installed):
    rows = []
    for path, raw in sorted(installed.items()):
        upstream = stable.PROJECTION_SOURCES.get(path, path.removesuffix(".upstream"))
        if path in (TRANSMUTE_PREFIX + "LICENSE-MIT", TRANSMUTE_PREFIX + "LICENSE-APACHE"):
            upstream = path.removeprefix(TRANSMUTE_PREFIX)
        row = {"path": path, "bytes": len(raw), "sha256": digest(raw)}
        if upstream in originals:
            row.update(upstream=upstream, upstream_sha256=digest(originals[upstream]))
        if path in stable.PROJECTION_SOURCES:
            row["mechanical_adapter"] = stable.ADAPTATION
        if path.startswith(TRANSMUTE_PREFIX) and path.removeprefix(TRANSMUTE_PREFIX) in transmute.PINS:
            row["mechanical_adapter"] = TRANSMUTE_ADAPTATION
        rows.append(row)
    return {
        "schema": 3, "repository": "https://github.com/rust-lang/rust.git",
        "commit": COMMIT, "crates": list(CRATES), "files": rows,
        "support_sources": list(SUPPORT_SOURCES),
        "packaging": "stable-shared-v2: retain all originals; registered " + stable.ADAPTATION + " compiler-helper projections and stable-transmute-v1 automaton packaging; retain upstream manifests; omit optional compiler-only dependencies; disable default/nightly feature activation",
        "rust_adaptation": stable.ADAPTATION,
        "transmute_adaptation": TRANSMUTE_ADAPTATION,
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
            or inventory.get("rust_adaptation") != stable.ADAPTATION
            or inventory.get("transmute_adaptation") != TRANSMUTE_ADAPTATION):
        raise ValueError("solver inventory pin/closure differs")
    originals = {path: (TREE / (path + ".upstream" if path in stable.RETAIN_ORIGINAL else path)).read_bytes()
                 for path in stable.HASHES}
    projections = stable.projections(originals)
    transmute_originals = {TRANSMUTE_PREFIX + name: transmute.original(name).encode() for name in transmute.PINS}
    transmute_projections = {TRANSMUTE_PREFIX + name: transmute.CARGO.encode() if name == "Cargo.toml" else transmute.stable(name, transmute.original(name)) for name in transmute.PINS}
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
        if path in transmute_projections:
            if (raw != transmute_projections[path]
                    or row.get("mechanical_adapter") != TRANSMUTE_ADAPTATION
                    or row.get("upstream") != path
                    or row.get("upstream_sha256") != digest(transmute_originals[path])):
                raise ValueError("registered transmute projection differs: " + path)
        elif path in projections:
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
            or not set(transmute_projections) <= expected
            or not {p + ".upstream" for p in transmute_originals} <= expected
            or not {p + ".upstream" for p in stable.RETAIN_ORIGINAL} <= expected):
        raise ValueError("solver source membership/licenses differ")
    changed = sum(projections[path] != originals[source]
                  for path, source in stable.PROJECTION_SOURCES.items())
    transmute_changed = sum(raw != transmute_originals[path]
                            for path, raw in transmute_projections.items() if path.endswith(".rs"))
    changed += transmute_changed
    return {"commit": COMMIT, "crates": len(CRATES), "files": len(expected),
            "source_bytes": total, "rust_source_changes": changed,
            "retained_original_rust_changes": 0,
            "transmute_rust_source_changes": transmute_changed,
            "registered_rust_adaptation": stable.ADAPTATION,
            "registered_transmute_adaptation": TRANSMUTE_ADAPTATION,
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

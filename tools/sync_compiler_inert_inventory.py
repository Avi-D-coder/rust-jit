"""Generate compiler metadata from the complete immutable pinned symbol table.

This deliberately accepts only that table's fixed data schema. The adapter's
pinned AST provenance control independently checks the complete generated set.
No guest source or macro expansion is parsed or executed by this tool.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / "fixtures/upstream/rustc-checkout"
SOURCE = "compiler/rustc_feature/src/builtin_attrs.rs"
COMMIT = "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
SHA256 = "a52761c0d4708b4282a9acc09447f11aa955a65289ace2cadabe1d788131a55c"
OUTPUT = ROOT / "crates/lrr-expand-adapter/src/compiler_inert_inventory.rs"
DIAGNOSTIC_PINS = ROOT / "tools/compiler_diagnostic_attribute_sources.json"
STATIC_PINS = ROOT / "tools/compiler_static_attribute_sources.json"


def static_feature_metadata(manifest):
    """Read every PATH in the pinned feature-allowance parser module.

    This is compiler metadata extraction, not a list selected by guest source
    names. Runtime-transforming parser modules are deliberately not admitted.
    """
    pins = json.loads(STATIC_PINS.read_text())
    if pins["schema_version"] != 1 or pins["commit"] != COMMIT:
        raise ValueError("static feature source pin schema or compiler differs")
    expected = "compiler/rustc_attr_parsing/src/attributes/allow_unstable.rs"
    if [pin["path"] for pin in pins["files"]] != [expected]:
        raise ValueError("unreviewed static feature parser category")
    names = set()
    sources = []
    digest = hashlib.sha256()
    for pin in pins["files"]:
        raw = (UPSTREAM / pin["path"]).read_bytes()
        actual = hashlib.sha256(raw).hexdigest()
        row = next(row for row in manifest["files"] if row["path"] == pin["path"])
        if (actual != pin["sha256"] or actual != row["sha256"]
                or len(raw) != pin["bytes"] or len(raw) != row["bytes"]):
            raise ValueError("static feature parser differs from its pinned import")
        text = raw.decode("utf-8")
        paths = re.findall(
            r"const PATH: &\[(?:rustc_span::)?Symbol\] = &\[sym::([a-z_]+)\];", text)
        if not paths or len(paths) != len(re.findall(r"\bconst PATH\b", text)):
            raise ValueError("static feature parser PATH schema changed")
        if len(paths) != len(set(paths)):
            raise ValueError("duplicate static feature parser PATH")
        names.update(paths)
        digest.update(pin["path"].encode() + b"\0" + actual.encode() + b"\0")
        sources.append((pin["path"], text))
    return sorted(names), digest.hexdigest(), sources


def diagnostic_metadata(manifest):
    """Read the whole pinned diagnostic parser registry, not an attribute allowlist."""
    pins = json.loads(DIAGNOSTIC_PINS.read_text())
    if pins["schema_version"] != 1 or pins["commit"] != COMMIT:
        raise ValueError("diagnostic source pin schema or compiler differs")
    directory = "compiler/rustc_attr_parsing/src/attributes/diagnostic/"
    sources = {}
    digest = hashlib.sha256()
    for pin in pins["files"]:
        path = pin["path"]
        if not path.startswith(directory) or Path(path).name in sources:
            raise ValueError("diagnostic registry source path differs or duplicates")
        raw = (UPSTREAM / path).read_bytes()
        actual = hashlib.sha256(raw).hexdigest()
        row = next(row for row in manifest["files"] if row["path"] == path)
        if (actual != pin["sha256"] or actual != row["sha256"]
                or len(raw) != pin["bytes"] or len(raw) != row["bytes"]):
            raise ValueError("diagnostic parser source differs from its pinned import")
        sources[Path(path).name] = (path, raw.decode("utf-8"))
        digest.update(path.encode() + b"\0" + actual.encode() + b"\0")
    modules = re.findall(r"^pub\(crate\) mod ([a-z_]+);$", sources["mod.rs"][1], re.M)
    if set(sources) != {"mod.rs", *(name + ".rs" for name in modules)}:
        raise ValueError("diagnostic parser registry is not complete")
    names = set()
    for module in modules:
        text = sources[module + ".rs"][1]
        paths = re.findall(r"&\[\s*(sym::[a-z_]+(?:\s*,\s*sym::[a-z_]+)*)\s*\]", text)
        if not paths:
            raise ValueError("diagnostic parser mapping schema changed")
        for path in paths:
            segments = re.findall(r"sym::([a-z_]+)", path)
            if len(segments) == 1:
                names.add(segments[0])
            elif len(segments) != 2 or segments[0] != "diagnostic":
                raise ValueError("diagnostic attribute path schema changed")
    return sorted(names), digest.hexdigest(), sorted(sources.values())


def generated():
    raw = (UPSTREAM / SOURCE).read_bytes()
    manifest = json.loads((UPSTREAM / "manifest.json").read_text())
    row = next(row for row in manifest["files"] if row["path"] == SOURCE)
    if (manifest["commit"] != COMMIT or hashlib.sha256(raw).hexdigest() != SHA256
            or row["sha256"] != SHA256 or row["bytes"] != len(raw)):
        raise ValueError("compiler attribute source does not match its pinned import")
    marker = "pub static BUILTIN_ATTRIBUTES: &[Symbol] = &[\n"
    text = raw.decode("utf-8")
    if text.count(marker) != 1:
        raise ValueError("compiler metadata declaration schema changed")
    body = text.split(marker, 1)[1].split("\n];", 1)[0]
    names = []
    for line in body.splitlines():
        entry = line.split("//", 1)[0].strip()
        if not entry:
            continue
        match = re.fullmatch(r"sym::([a-zA-Z_][a-zA-Z_0-9]*),", entry)
        if not match:
            raise ValueError("compiler symbol metadata schema changed")
        names.append(match[1])
    if not names or len(names) > 4096 or len(set(names)) != len(names):
        raise ValueError("compiler metadata empty, duplicate or oversized")
    diagnostic_names, diagnostic_digest, diagnostic_sources = diagnostic_metadata(manifest)
    static_names, static_digest, static_sources = static_feature_metadata(manifest)
    if not set(diagnostic_names) <= set(names):
        raise ValueError("diagnostic compiler alias absent from complete inventory")
    if not set(static_names) <= set(names) or set(static_names) & set(diagnostic_names):
        raise ValueError("static feature category absent or overlapping diagnostic inventory")
    lines = [
        "// Generated by tools/sync_compiler_inert_inventory.py; do not hand-edit.",
        "// Full pinned compiler metadata; no guest expansion/execution semantics.",
        f'pub(super) const COMPILER_COMMIT: &str = "{COMMIT}";',
        'pub(super) const SOURCE_SHA256: &str =',
        f'    "{SHA256}";',
        "pub(super) const NAMES: &[&str] = &[",
        *(f'    "{name}",' for name in sorted(names)),
        "];",
        'pub(super) const DIAGNOSTIC_SOURCE_IDENTITY: &str =',
        f'    "{diagnostic_digest}";',
        "#[rustfmt::skip]",
        "pub(super) const DIAGNOSTIC_NAMES: &[&str] = &[",
        *(f'    "{name}",' for name in diagnostic_names),
        "];",
        "#[cfg(test)]",
        "#[rustfmt::skip]",
        "pub(super) const DIAGNOSTIC_SOURCES: &[(&str, &str)] = &[",
        *(f'    ("{path}", include_str!("../../../fixtures/upstream/rustc-checkout/{path}")),'
          for path, _ in diagnostic_sources),
        "];",
        'pub(super) const STATIC_FEATURE_SOURCE_IDENTITY: &str =',
        f'    "{static_digest}";',
        "#[rustfmt::skip]",
        "pub(super) const STATIC_FEATURE_NAMES: &[&str] = &[",
        *(f'    "{name}",' for name in static_names),
        "];",
        "#[cfg(test)]",
        "#[rustfmt::skip]",
        "pub(super) const STATIC_FEATURE_SOURCES: &[(&str, &str)] = &[",
        *(f'    ("{path}", include_str!("../../../fixtures/upstream/rustc-checkout/{path}")),'
          for path, _ in static_sources),
        "];", "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = generated()
    if args.check:
        if OUTPUT.read_text() != expected:
            raise SystemExit("compiler inert metadata differs; regenerate from pinned source")
        print("Complete pinned compiler inert metadata is current; no runtime acceptance claimed")
    else:
        OUTPUT.write_text(expected)
        print("Generated complete pinned compiler metadata; adapter AST validation pending")


if __name__ == "__main__":
    main()

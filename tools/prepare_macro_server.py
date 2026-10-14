"""Explicit preparation of the pinned Rust1.99 macro-host server; no live unit build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from bootstrap_vendor import apply_text_patch

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "tools/prepared_macro_server"


def sha(path):
    if path.stat().st_size > 1024 * 1024 * 1024:
        raise ValueError("prepared tool provenance file exceeds 1GiB")
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-tool-preparation", action="store_true")
    parser.add_argument("--receipt-directory", type=Path)
    args = parser.parse_args()
    if not args.allow_tool_preparation:
        parser.error("explicit tool-preparation authority is required")
    audit = subprocess.run(["python3", str(ROOT / "tools/bootstrap_vendor.py")],
                           check=True, capture_output=True, text=True)
    vendor = json.loads(audit.stdout)
    entry_target = "crates/proc-macro-srv-cli/src/main.rs"
    original_entry = ROOT / "vendor/rust-analyzer-macros" / entry_target
    entry_patch = PACKAGE / "stdio-v1.patch"
    assert apply_text_patch(original_entry.read_bytes(), entry_patch.read_bytes(), entry_target) == (PACKAGE / "src/main.rs").read_bytes(), "prepared entry differs from versioned pinned-source patch"
    version = subprocess.check_output(["rustc", "+1.99.0", "-vV"], text=True)
    assert "release: 1.99.0\n" in version and "host: aarch64-apple-darwin\n" in version
    commit = next(line.removeprefix("commit-hash: ") for line in version.splitlines()
                  if line.startswith("commit-hash: "))
    sysroot = Path(subprocess.check_output(["rustc", "+1.99.0", "--print", "sysroot"], text=True).strip())
    component = sysroot / "lib/rustlib/manifest-rustc-dev-aarch64-apple-darwin"
    if not component.is_file():
        component = sysroot / "lib/rustlib/manifest-rustc-dev"
    assert component.is_file(), "exact Rust1.99 rustc-dev preparation dependency is missing"
    env = os.environ.copy()
    env.update(RUSTC_BOOTSTRAP="1", CARGO_PROFILE_DEV_DEBUG="0", CARGO_INCREMENTAL="0")
    if args.receipt_directory is not None:
        assert args.receipt_directory.is_absolute() and args.receipt_directory.is_dir()
    log = (args.receipt_directory / "server-build.log" if args.receipt_directory is not None
           else ROOT / "artifacts/macos-first/m013-prepared-server-repro-build.log")
    with log.open("w") as output:
        subprocess.run(["cargo", "+1.99.0", "build", "--locked", "--offline",
                        "--manifest-path", str(PACKAGE / "Cargo.toml"),
                        "--target-dir", str(PACKAGE / "target")],
                       env=env, stdout=output, stderr=subprocess.STDOUT, check=True)
    binary = PACKAGE / "target/debug/rust-analyzer-proc-macro-srv"
    banner = subprocess.check_output([str(binary), "--version"],
                                    env={"RUST_ANALYZER_INTERNALS_DO_NOT_USE": "explicit LRR tool preparation"}, text=True)
    assert banner.startswith("rust-analyzer-proc-macro-srv 1.99.0 ") and commit in banner
    assert vendor["commit"] in banner and "lrr-stdio-v1" in banner
    source_files = [PACKAGE / name for name in ["Cargo.toml", "Cargo.lock", "build.rs", "src/main.rs", "stdio-v1.patch"]]
    source_files += [ROOT / "tools/prepare_macro_server.py", ROOT / "vendor/UPSTREAM.toml"]
    receipt = {
        "schema": 1, "role": "proc-macro-host-server", "platform": "aarch64-apple-darwin",
        "compiler_version": version, "compiler_commit": commit, "compiler_sysroot": str(sysroot),
        "compiler_dev_manifest": str(component), "compiler_dev_manifest_sha256": sha(component),
        "pinned_vendor": vendor, "server": str(binary), "server_sha256": sha(binary),
        "entry_origin": {"path": str(original_entry.relative_to(ROOT)), "sha256": sha(original_entry), "patch_sha256": sha(entry_patch)},
        "build_authority": "explicit --allow-tool-preparation", "rustc_bootstrap_scope": "compiler-private external macro-host server only",
        "server_version": banner.strip(), "protocol": 6, "diagnostic_stdout": "stderr",
        "source_sha256": {str(p.relative_to(ROOT)): sha(p) for p in source_files},
        "root_lock_sha256": sha(ROOT / "Cargo.lock"), "build_log_sha256": sha(log),
        "live_runtime_unit_compiled": False, "production_adoption": False,
        "full_fixture_acceptance": False,
    }
    path = (args.receipt_directory / "server-preparation.json" if args.receipt_directory is not None
            else ROOT / "artifacts/macos-first/m013-prepared-server-preparation.json")
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"receipt": str(path), "server": str(binary), "compiler_commit": commit,
                      "production_adoption": False}))


if __name__ == "__main__":
    main()

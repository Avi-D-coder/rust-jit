# Dependency lock provenance

The workspace uses every exact direct dependency from specification Section 3.2,
Rust 1.99.0, and resolver 3. Cargo.lock fixes the complete transitive graph.
The retained Cranelift JIT archive and its SHA-256 are in vendor/UPSTREAM.toml.
The only upstream source change clears fallback symbol lookup. Verify all vendor
bytes with `python3 tools/vendor_audit.py` or `cargo run --locked -p xtask -- vendor-audit`.

The initial resolution selected unicode-ident 1.0.26 (Unicode 18) alongside
unicode-properties 0.1.4 (Unicode 17). The pinned ra-ap-rustc_lexer 0.166.0
has a compile-time assertion requiring their Unicode versions to match; the
first host build failed with E0080 at its src/lib.rs:47. Resolution was repaired
with `cargo +1.99.0 update -p unicode-ident --precise 1.0.24 --offline`.
The latter's src/tables.rs declares Unicode 17, matching unicode-properties.
No direct dependency requirement or backend was changed. The successful
parser bootstrap regression and the checked lockfile preserve this repair.

Rust 1.99.0 was installed locally; release-target validation uses the official
linux/amd64 rust:1.99.0-bookworm image (digest
sha256:59037199c44290f2befcdd58dcc540164763fc296950255aaefeef096a1866b0).
Container execution is emulated on an ARM64 macOS host. Performance measurements
from this environment cannot establish native x86-64 latency targets.

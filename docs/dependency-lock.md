# Dependency lock provenance

The workspace uses every exact direct dependency from specification Section 3.2,
Rust 1.99.0, and resolver 3. Cargo.lock fixes the complete transitive graph.
The retained Cranelift JIT archive and its SHA-256 are in vendor/UPSTREAM.toml.
The only Cranelift upstream source change clears fallback symbol lookup. Verify all vendor
bytes with `python3 tools/vendor_audit.py` or `cargo run --locked -p xtask -- vendor-audit`.

The initial resolution selected unicode-ident 1.0.26 (Unicode 18) alongside
unicode-properties 0.1.4 (Unicode 17). The pinned ra-ap-rustc_lexer 0.166.0
has a compile-time assertion requiring their Unicode versions to match; the
first host build failed with E0080 at its src/lib.rs:47. Resolution was repaired
with `cargo +1.99.0 update -p unicode-ident --precise 1.0.24 --offline`.
The latter's src/tables.rs declares Unicode 17, matching unicode-properties.
No direct dependency requirement or backend was changed. The successful
parser bootstrap regression and the checked lockfile preserve this repair.

Rust 1.99.0 was installed locally; primary implementation and acceptance now run
on macOS. Deferred Linux-port validation can use the official
linux/amd64 rust:1.99.0-bookworm image (digest
sha256:59037199c44290f2befcdd58dcc540164763fc296950255aaefeef096a1866b0).
Container execution is emulated on an ARM64 macOS host. Performance measurements
from this environment cannot establish native x86-64 latency targets.

Revision 4 also retains the exact rust-analyzer macro source archive, inventory,
workspace manifests, lockfile, and licenses. Its adapter dependency closure adds
49 locked packages without changing the runtime's exact direct versions. Private
upstream Salsa is confined to that adapter closure. See macro-bootstrap.md and
`python3 tools/bootstrap_vendor.py` for provenance and boundary evidence.


The pinned macro snapshot now includes the declared E-M006-001 transcriber
cardinality correction. `vendor/mbe-cardinality.patch` is applied to the exact
archived preimage; `vendor/UPSTREAM.toml` records patch/preimage/postimage SHA-256.
The original archive and original file inventory are preserved. Bootstrap audit
derives the patched inventory in memory and rejects every other working-tree
modification. See docs/errata/m006-repetition-cardinality.md and the retained
native/backend witnesses. No dependency version or frontend was replaced.

M006 now also applies E-M006-002 repetition termination and E-M006-003 explicit
token preflight. The transcriber patch is combined against its original archived
preimage. Three separately audited supporting patches expose the existing `tt`
restore-point type, add the bounded `mbe` entry point, and thread its limit and
telemetry through rule dispatch. The original archive/inventory, selected
frontend and exact dependency versions remain unchanged. Audit reconstructs all
four modified files; forged patch/preimage/postimage receipts for every file are
covered by the patch-proof regression. Runtime frontend/Salsa isolation remains
unchanged; only the private adapter calls the bounded upstream API.

E-M006-004 extends those same original-preimage patches with exact schema2 byte
admission through captured owned span/context costs. Context/span field sizes are
measured by shared codec writers; literal spelling uses the pinned formatter
with a checked length sink. No dependency pin or archive inventory changes.
All final patch receipts are in `vendor/UPSTREAM.toml`; byte/scratch/hygiene
coverage is reported separately in the M006 checkpoint.

E-M006-005/006 extend the same audited four-file set with admitted iteration
replay, checked temporary raw-slot admission, and ordinary stable-definition
syntax rejection. The native scratch/profile witnesses and final checkpoint
preserve exact scope; all original archive/inventory hashes remain unchanged.

M007 adds exact supplemental `unicode-normalization = 0.1.25` for owned NFC
macro name identity, with Unicode 17 tables. Section 3.2 records the pin;
all earlier direct pins remain unchanged. The locked support closure adds
`tinyvec 1.13.2` and `tinyvec_macros 0.1.1`. The existing unicode-ident 1.0.24
compatibility repair is retained. See docs/errata/m007-unicode-names.md;
namespace normalization and macro token-input normalization have separate scopes.

M009 adds exact Rustix1.1.5 fs/process feature access for safe owned-helper pipe flags, group termination and fixture liveness probes. That version was already locked; its pinned registry source was inspected before API use. Standard-library child ownership, threads/channels/timers and the selected macro/native/sandbox architecture remain unchanged. E-M009-001 records the actual macOS inherited-pipe witness and the narrow Section3.2 exception.

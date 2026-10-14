# E-M010-001: required compiled helper metadata is absent from the matched protocol

Status: protocol gap confirmed; captured original-source facets now provide a positive metadata path. Full generated/mixed-edition helper support remains mandatory.
This erratum records the failed assumption that the export-list protocol suffices
for helper metadata. It does not prove that no architecture-compatible preparation
metadata path exists. It grants no permission to replace the
frontend, compiler tuple, protocol implementation or supplied server.

Sections 48.6 and 49.4 jointly require the supplied Rust 1.99.0 matched server
and helper names from each exact loaded derive export. The pinned API6
bidirectional `Response::ListMacros` is `Vec<(String, ProcMacroKind)>`.
`proc-macro-srv/src/dylib/proc_macros.rs::list_macros` projects custom derives to
trait name and kind. There is no helper-attribute field in this wire inventory.
Rust's acceptance of a helper cannot be inferred from an empty field that does
not exist.

Minimal witness: one standalone controlled proc-tool source uses macro_rules to
generate `#[proc_macro_derive(Identical, attributes($helper))]`. Feature first
compiles `first_helper`; feature second compiles `second_helper`. Explicit pinned
Cargo preparation builds only this proc-macro unit, with an offline lockfile.
Both exact artifacts are loaded only in authorized supplied-server sessions.
Their inventories are identical (Identical, derive), with no helper metadata.
Four standalone native compiler observations prove distinct acceptance sets:
first accepts first_helper and rejects second_helper; second does the reverse.
Rejections must contain the unknown-attribute diagnostic, and accepted compiles
must succeed. The library hashes differ. No macro is invoked through the LRR
sessions while reading inventories. Native compiler observations are separate
synthetic oracle runs, not a runtime fallback.

Regression: `matched_server_export_inventory_cannot_infer_generated_derive_helpers`
retains sources, compiler output, exact artifact and server hashes,
compiler commit, native diagnostics and `witness.json` under the generated
`artifacts/macos-first/m009-proc-session-*/helper-metadata-witness/` directory.
The actual prepared-service test additionally checks that required helper metadata
returns LRR-E-MACRO-SERVER-MISMATCH with zero session invocations, and stale
inventory context refuses.

Correction to evidence and failure handling: advertise only observed capabilities.
The retained export facet distinguishes NotDerive from Unavailable(protocol=6).
Unavailable is not an empty helper list and cannot establish a negative helper
lookup. Pure item helper lookup stays NeedDeclarations until exact metadata exists;
the service reports the missing required server capability rather than guessing
from source spelling, library hash, or static unexpanded declarations.

Resolution still needs an exact-authority metadata path compatible with the
specified tuple and architecture, plus generated/conditional declaration fixtures.
Captured original-function source facets now pass preparation/unit/artifact/hash/cfg
admission and real native helper comparisons; see [source facet evidence](../source-derive-declarations.md). Generated declarations, complete catalogs, outside-root
sources and generated/mixed-edition execution still need their frontend/capture integration. M010,
MX018–MX020 and release helper acceptance remain incomplete. This witness is component capability
measurement and regression evidence; it must never be used as a passed full helper
fixture. Other supported macro operations retain their separate measured scope.

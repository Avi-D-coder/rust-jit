# M007 revision 4 acceptance audit

M007 component acceptance is complete on aarch64-apple-darwin with Rust 1.99.0.
Release acceptance remains false. This follows the Section 57 dependency order:
M008 can now consume the implemented hygiene and namespace interfaces.

| Required behavior | Actual evidence |
| --- | --- |
| Textual source order, captured parent scope and shadowing | Original entered-body native comparison and deterministic scope tests |
| Path namespaces, renames and reexports | Original-source import/export routing, alias/glob/privacy fixtures |
| $crate defining-unit authority | MX011: actual Cargo provider/facade/caller programs and pinned marked expansions, including raw/NFC source names |
| Raw identifiers and owned pinned name contexts | Parser/lexer validation, normalization and actual marked transport/native fixtures |
| Definition/call-site local collision | Original source binding/rib selection with actual pinned mark contexts versus native values |
| Generated local collision plus textual shadowing | MX012: one native program; four original-source textual selections/pinned expansions and two marked/caller local observations |
| Pending inputs, atomic supplied declaration batches and origin ambiguity | Typed kernel and source-index acceptance; stale/unknown/duplicate/context/range/ancestry refusal checks |

The machine-readable audit enumerates all 160 ledger acceptance entries and all
126 M007 registry references and checks each against actual successful host test
output. Seventeen previously unregistered ledger IDs were repaired; ledger
validation now rejects unknown, duplicate and non-string fixture references.
The regression itself passes. No selected fixture was skipped or converted into
success by swallowing errors. The milestone gate passes with 328 host/doc tests,
strict Clippy and formatting.

The older textual-only report's supplemental expanded-macro E0659 source
integration probe remains explicitly unverified. It is not used as proof of
completion here. The owned origin ambiguity tests and the named MX011/MX012
cases have separate actual evidence.

This closes the task card's hygiene/namespace component, not the full Rust
profile or executable runtime. General generated AST discovery, complete body
forms, nested generated scopes, coordinator epochs/output attestation and saved
VM phases belong to later T/M cards. Interpreter/JIT/native runtime, release
security/performance and Linux port acceptance remain pending. Historical
component notes are preserved in the ledger; its current validation_scope and
this audit state the final M007 position.

Evidence: artifacts/macos-first/m007-final-acceptance-audit.json.

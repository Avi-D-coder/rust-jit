# E-M015-003: crate cfg exclusion precedes original test discovery

The unchanged official rustc `tests/ui/test-attrs/issue-34932.rs` begins with
`#![cfg(false)]`. Its `dummy` test contains a deliberately invalid type expression
that must be configured away. With a disclosed Rust 2024/test-cfg metadata
adaptation, the actual LRR discovery coordinator sealed and published a catalog
containing `dummy`. The expected empty final catalog assertion failed after an
independent zero body/signature/thunk demand check. This is a catalog failure,
not an inference from cold parser inventory. No native preparation or selected
live compilation ran.

The witness uses 297 production/package files verified byte-for-byte against
the integrated 1126-input macOS source snapshot, with all 3300 before/after
candidate inputs unchanged. Earlier failures from an overstrong intermediate
inventory-empty assertion are retained separately. The current final-catalog
expectation remains strict. The receipt corrects a metadata-only 298/297 typo
while preserving the original record.

The declaration scanner and derive catalog already applied source-file cfg.
The original outline and structural module inventory started at child items and
missed root inner cfg. The proposed repair shares bounded pure root-attribute
preprocessing with those paths. A disabled root returns an empty outline and no
external-module demands; inline caller projection explicitly refuses that root.
The repair preserves the pinned frontend, cfg interpreter, capabilities and
language profile. Independent worker validation of the unchanged final-catalog regression passes
after copying only the three frozen parent syntax files: a real empty sealed
catalog, repeated reads and zero body demands. Parent V2 broader component checks passed: two root-cfg, 174 library, 65 scan
and seven enclosing controls, formatting, strict workspace Clippy and architecture.
The initial supplementary active-source assertion omitted the retained Import
header; its failure is preserved and the successor explicitly asserts all three
source declaration kinds. Production repair bytes stayed unchanged. The joint native namespace/disabled-producer control passes, and the integrated
macOS milestone is preserved in m015-cfg-facade-main-acceptance.json. Full
fixture and generated-module authority acceptance remain pending.

Confirmation: `artifacts/macos-first/m015-root-cfg-worker-pure-confirmation.json`.

Witness: `artifacts/macos-first/m015-root-cfg-worker-witness.json`.
Immutable bundle: `artifacts/macos-first/m015-root-cfg-worker-witness-evidence.tar`.
Parent source: `artifacts/macos-first/m015-root-cfg-v1-source-snapshot/manifest.json`.

This is M015 component work. The official run-pass bodies, full stock fixture,
guest JIT, full MX031/MX032, release and Linux acceptance remain pending.

Parent component receipt: `artifacts/macos-first/m015-root-cfg-v2-acceptance.json`.

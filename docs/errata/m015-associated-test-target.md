# E-M015-007: associated compiler Test attributes must reach discovery

Task M015 implements revision4 section50.4 final test metadata and required
transformation failure. This correction concerns declaration target eligibility;
ordinary parameter, generic and result signatures remain deferred by25.2/50.4.

The unchanged official `tests/ui/test-attrs/test-attr-non-associated-functions.rs`
at rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18 contains two `#[test]`
associated functions and one ordinary test. The worker's real LRR Rust2024/test
adaptation produced a sealed catalog containing only `test`, without inspecting
signatures or bodies. Stock expected stderr rejects both associated targets.
This is a compiler Test handling gap, not permission to eagerly type-check cold
signatures. Full stock stderr matching and execution remain unexercised.

Minimal LRR witness: `struct A; impl A { #[test] fn recovery_witness() {} }
#[test] fn known() {}`. Discovery silently sealed; its strict primary-refusal
regression failed before the correction. The first guard-only repair still
failed: attributes on associated headers were delegated because their executable
owner was absent, while the ordinary outline has no associated item owner.
Both failed candidates and unchanged source snapshots are preserved.

Correction: use exact existing associated lexical-scope identities to retain
associated attributes on the declaration worklist. Only after cold resolution
selects the retained compiler Test facet, check the exact captured attribute's
AST attachment. Its direct owner must be a free function, not an associated
method or other declaration. Cfg-disabled subtrees stay excluded. Validation
reads no function signature, body or thunk and executes no macro host. A
mandatory target error is retained before inert retirement or seal publication.
Procedural attributes resolved from the same spelling retain their separate
selection path; a name alone is not a compiler authorization.

Acceptance: minimal primary-error replay, no seal/no database revision mutation,
trait and effective cfg_attr associated targets, disabled associated control,
unchanged full original associated fixture, and original/generated selected
free-signature-prefix controls. Generated associated target completion and other
non-function target producers have separate prerequisites; no full target or
stock pass may be inferred from this bounded original cold-attribute repair.

Integrated macOS milestone passed1986host/doc checks with all9checks0 against
3486frozen source/include inputs and unchanged membership. See
artifacts/macos-first/m015-associated-target-main-acceptance.json.
M015 remains in progress; original non-function and generated target refinements
are separate private candidates.

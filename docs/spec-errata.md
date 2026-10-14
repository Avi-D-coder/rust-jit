# Specification errata index

Each entry records its witness, correction and acceptance scope. This index
does not imply completed task or release acceptance. Patch receipts and frozen
platform checkpoints provide the corresponding implementation evidence.

- [E-M005-001: pinned raw token converter doc-comment panic](errata/m005-doc-comment-converter.md)
- [E-M006-004: exact schema2 byte admission](errata/m006-byte-preflight.md)
- [E-M006-001: unequal declarative repetition cardinalities](errata/m006-repetition-cardinality.md)
- [E-M006-005: admitted replay without discarded iteration output](errata/m006-scratch-replay.md)
- [E-M006-006: stable ordinary macro definitions](errata/m006-stable-profile.md)
- [E-M006-003: token admission inside the pinned transcriber](errata/m006-token-preflight.md)
- [E-M007-003: complete reexport cycles are empty](errata/m007-complete-reexport-cycles.md)
- [E-M007-001: expose the pinned mark algorithm without a source query](errata/m007-hygiene-api.md)
- [E-M007-005: preserve the producer's source interval](errata/m007-local-publication-order.md)
- [E-M007-002: marked transcription and ancestor byte accounting](errata/m007-marked-byte-preflight.md)
- [E-M007-004: owned macro names need NFC identity](errata/m007-unicode-names.md)
- [E-M007-006: preserve emitted bindings through hygiene filtering](errata/m007-textual-binding-history.md)
- [E-M007-007: include transforming scope-opening demands](errata/m007-source-opening-frontier.md)
- [E-M007-008: preserve raw macro source spelling through validation](errata/m007-raw-source-name.md)
- [E-M007-009: entered statement frontiers and opaque ranges](errata/m007-entered-call-frontiers.md)
- [E-M007-010: validate every inherited declaration row](errata/m007-source-chain-metadata.md)

## E-M007-011: Macro reexport visibility

Native Rust 1.99.0 macOS rejects public reexport of a private imported macro alias
with E0364. Caller-aware traversal now checks reexport width as well as access
from the import origin. See [the witness and regression](errata/m007-reexport-visibility.md).

- [E-57-001: fixed-order component and later integration acceptance](errata/e57-component-integration-order.md)

- [E-M010-001: required compiled helper metadata is absent from matched API6](errata/m010-compiled-helper-metadata.md) — protocol gap confirmed; source-facet integration progresses and full helper acceptance remains mandatory.

- [E-M011-001: syntax-only pinned concat API](errata/m011-concat-api.md)
- [E-M011-002: numeric concat spelling contradiction](errata/m011-concat-numeric-spelling.md)

- [E-M011-003: physical versus virtual include file bases](errata/m011-include-virtual-base.md) — native file-symlink witness and corrected component regressions.

- [E-M013-001: native stdout corrupts the matched binary macro protocol](errata/m013-procedural-stdout.md) — unchanged original auxiliary witness; terminal refusal regression only, procedural fixture execution remains failed/pending.

- [E-M015-001: generated physical roots retain exact insertion attachment](errata/m015-generated-root-attachment.md) — matched grammar witness and owned module admission regression; route/coordinator acceptance pending.

- [E-M015-002: generated module names retain token resolution editions](errata/m015-generated-module-token-edition.md) — mixed definition/caller and relocated-span regressions; runtime outline and route integration pending.

- [E-M015-003: root cfg precedes test discovery](errata/m015-root-cfg.md) — unchanged final catalog regression and native disabled-producer control.
- [E-M015-004: generated extern crates retain owned compiler facets](errata/m015-generated-compiler-facade.md) — matched declarative and procedural witnesses; bounded shared alias repair.

- [E-M015-005: stable expected-panic name-value strings](errata/m015-expected-panic-name-value.md) — exact original/generated metadata witness and unchanged repaired assertions.

- [E-M015-006: rejected owned items retain discovery failure](errata/m015-required-item-failure.md) — actual ordinary/generated reply witnesses, foreign-request controls and retained budget accounting.

- [E-M015-007: associated compiler Test targets reach discovery](errata/m015-associated-test-target.md) — unchanged original witness, exact target refusal and deferred-signature controls; generated target prerequisites remain pending.

- [E-M015-008: original and generated compiler Test target eligibility](errata/m015-generated-test-target-refinements.md) — exact target ownership, compiler binding, primary refusal retention and unchanged upstream controls.

- [E-M015-009: procedural Test candidates use both catalog startup paths](errata/m015-procedural-test-startup.md) — actual early-path misregistration witness, shared reservation helper and independently tested early/late/non-function controls.

- [E-M015-010: attribute output retains its captured declaration grammar](errata/m015-associated-attribute-grammar.md) — retained strict witness and integrated 2042-test macOS milestone acceptance; full fixture acceptance pending.

- [E-M015-011: generated compiler Test eligibility uses exact lexical attachment](errata/m015-generated-associated-lexical-test-target.md) — retained strict witness and integrated 2042-test macOS milestone acceptance; full fixture acceptance pending.

- [E-M015-012: associated producers retain declaration ownership](errata/m015-original-associated-ownership.md) — retained strict witness and integrated 2042-test macOS milestone acceptance; full fixture acceptance pending.

- [E-M015-013: generated attributes bind in their captured declaration grammar](errata/m015-generated-attribute-binding.md) — retained strict witness and integrated 2042-test macOS milestone acceptance; full fixture acceptance pending.

- [E-M015-014: original doc comments keep their lexical attribute positions](errata/m015-original-doc-attribute-order.md) — unchanged auxiliary ordering witness, two actual repaired callbacks and integrated 2046-test macOS milestone; full fixture acceptance pending.

- [E-M015-015: builtin-inert attributes require exact Attribute-facet resolution](errata/m015-inert-attribute-ambiguity.md) — preserved actual original/generated/cold witnesses and repaired independent native controls; first main gate failed ENOSPC, combined corrected milestone pending.

- [E-M015-016: compiler-owned Attribute aliases require discovery on both startup paths](errata/m015-compiler-inert-attribute-alias.md) — actual original/generated/cold and late-output witnesses, exact facet/startup correction and independent strict acceptance; fresh combined macOS milestone pending.

- [E-M015-017: selected compiler Test action](errata/m015-compiler-test-action.md): actual CLI and strict stock-body adaptation witnesses; private repair under validation.
- [E-M015-018: generated inert meta path ownership](errata/m015-generated-inert-path-owner.md): two collected main compatibility failures; first correction leaves one root-inner failure; revised private validation pending.

- [E-M015-019: retain stock support inputs and rustfix expectations](errata/m015-stock-fixture-closure.md).

- [E-T041-001: continuation-only recipes have zero ordinary operations](errata/t041-continuation-operation-count.md) — retained unchanged stock dereference failure; repair unvalidated.

- [E-T043-001: locals retain primary declaration drop order](errata/t043-local-declaration-drop-order.md) — unchanged original/native witness confirmed; private LRR regression unvalidated.

- [E-NUM-001: user-authorized Rust-compatible numeric inference](errata/numeric-rust-compatibility.md)
\n- [E-EDITION-001: all released official Rust editions](errata/all-official-editions.md) — user-authorized root scope expansion; original 2015 macro revalidation witness retained, edition acceptance pending.\n
- [E-R5-001: ABI/encoding boundaries and revision-5 integration consistency](errata/revision5-boundary-consistency.md) — documented contradictions, static mutation regressions and pending runtime acceptance; interpreter-suite policy clarification remains outstanding.

- [E-R5-002: interpreter decoding, tier transfer, logical quotas and bounded reconsideration](errata/revision5-tier-transfer.md) — requirements adopted; runtime acceptance pending.

- [Numeric local-literal fallback regression](errata/numeric-local-literal-fallback.md) — implementation correction under Rust compatibility; whole stock const qualification and fresh integrated milestone remain pending.

- [Direct numeric source constraints and hidden let-else](errata/numeric-direct-local-constraints.md) — bounded source/borrow repair; full inference and compiletest acceptance pending.

- [Normalized deferred return source](errata/normalized-deferred-return-context.md) — minimal primitive-looking alias witness, private current-source admission and scoped literal result retention; full alias bodies pending.

- [Intrinsic literal source and reached boundary context](errata/intrinsic-literal-source.md): preserve actual Bool/Char producers without treating propagated expected context as an already produced value. Component validation and MAIN integration pending.

- [E-R5-003: backing adoption, compiler result disposal, scheduling policy and full-suite accounting](errata/revision5-final-contracts.md) — requirements adopted; implementation pending.

- [E-R5-004: runtime-semantics prerequisite; static checking stays external](errata/revision5-runtime-acceptance.md) — supersedes the all-category pre-JIT gate.

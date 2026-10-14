# Primitive-looking aliases retain their normalized return source

The minimal witness is `type i64=bool;fn main(){chosen();}fn chosen()->i64{true}fn cold(){}`. Frontend alias normalization correctly selected bool, but frame capture discarded that deferred result and the root thunk reapplied the printed i64 hint. The original program then faulted with CALL-RETURN-TYPE. This contradicts the required source-aware return contract; it is an implementation correction, not an architecture change.

The candidate frontend mints a private certificate after successful owning alias normalization, binding the declaration/signature, written hint, resolved primitive/descriptor, exact source/query epochs and source/alias dependency stamps. Current-source frame admission checks the immutable certificate against the public packet before copying it. A matching descriptor alone cannot provide source identity. Legacy capture rejects such certificates without current admission.

Only the exact admitted root hint can use the normalized context. Root result transfer preserves the declaration source and refuses retained rigid, nominal or wrong primitive sources. An absent source may be recovered only for a primary indexed primitive literal at a matching scalar descriptor; a variable or caller value with missing identity remains refused. General alias bodies and source propagation are separate interpreter work.

Seven scoped controls passed on frozen private source, including actual all-edition witness execution, stale same-header aliases, derived publication changes, wrong-owner/forged packet fields, equal-layout rigid refusal and missing-variable identity refusal. The original nominal bool header control requires actual compiler-inert attribute/declaration preparation; no declaration was manually restored. Existing symbolic return and source-run controls are being validated. Full rustc compiletest passes remain zero, MAIN integration and the milestone remain pending, and no task is complete.

Evidence: `artifacts/macos-first/root-return-and-attribute-v1`. Failed baseline and harness/admission migrations remain preserved.

## Transitive source-world review

The initial certificate passed 541 unfiltered library bodies, 28 CLI bodies, strict workspace Clippy and formatting on frozen private source. Integration is held for an additional review witness: an unchanged root alias `type R=B` importing `types::B` may retain its immediate recipe while a child module changes `B` in a fresh database with the same local revision counters. Current epochs are not a substitute for transitive owning source identity. The regression reproduced actual stale admission before repair. A second unchanged-owner witness then reproduced stale bridge reexport routing being stamped as current. The repairs retain each traversed alias source/header/context/cfg/query stamp and conservatively bind the captured namespace source catalogue, verifying that the supplied selections match publication before stamping. Both red runs are preserved. Current validation is pending; the initial passing component receipt does not establish these additional boundaries.

The existing 256-step normalization walk remains demand driven. Its retained trace allows 4,000 rows and 32 MiB; the merged private certificate allows 4,096 rows and 32 MiB, checks arithmetic, canonical ordering, duplicate/conflicting keys and stable v2 encoding. These limits are component safeguards, not full trait or generated-namespace acceptance.

## Integrated component checkpoint

The final combined frozen component run passed 552 library, 166 focused integration and 28 CLI bodies plus strict workspace Clippy and formatting. Required source/identity/fuel/limit regressions passed; original failures and exact manifests are retained. Tested code and assertions are integrated in MAIN. The new macOS milestone is pending, affected task requirements remain in_progress, and full rustc compiletest passes remain zero. See artifacts/macos-first/root-alias-transitive-trace-v1/combined-component-acceptance-v2.json and root-return-and-attribute-v1/MAIN-integration-v1/integration.json.

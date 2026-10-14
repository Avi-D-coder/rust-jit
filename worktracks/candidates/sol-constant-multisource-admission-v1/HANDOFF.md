# T054 / WT02-04 exact current multisource constant admission

Frozen source-only successor of sol-constant-macro-admission-v1. Preserved prerequisites: sol-constant-source-admission-v1, sol-constant-path-selection-v1 and macro packet are unchanged. Owned production: frontend/constant_source_admission.rs plus new private child frontend/constant_source_admission_sources.rs. Root evaluator, Node evaluation, constants/wire/memo, DB keys/providers/memo, query publication/runtime and canonical source/targets are untouched.

## Concrete root API

`ConstantSourceAdmission::capture_current_in_sources(db, target_view, definition, request, byte_limit, &[&PreparedSourceView]) -> Result<Self, Diagnostic>`.

Existing `capture_current` and `capture_current_with_macro_proofs` retain their signatures and delegate the exact singleton path. The privately minted admission retains immutable original views; `revalidate(db,target_view)` retains its signature and rebuilds against that exact retained set. No DB selection or provider is invoked. Root must pass the actual retained originals selected by preparation/current path evidence, publish both namespaces over that set outside queries, validate before publishing/reusing any result and observe every `dependencies()` entry. The source views are evidence only: supplying a list never asserts completeness or authority.

## Admission and identity

The complete first lane equals all actual currently published CapturedSource rows (bounded by the existing current-input accessor and a new256-source bound). Each canonical selection/location and original parse/hygiene/declaration context must be retained and its prepared source/cfg/compiler/declaration facts must match current published inputs. Duplicate or omitted source locations, extra rows, changed facts or an absent target's exact source lease refuse. Distinct original leases are allowed when actual facts match; each unique lease's prepared index metadata is charged once. Retained selections are canonicalized by location identity, preserving path, source content/profile/owning edition/cfg, unit and module context.

Both actual Type and Value seals must disclose exactly that same complete source set, exact target module scope/namespace/owning edition, zero unresolved frontiers, current world/epoch/source graph and individually current compiler inputs. For the multi-source lane, compiler-input sets must additionally agree and both private CertifiedScopes must equal fresh original NameNamespaces replay. Dependency aliases are taken solely from the actual current SourceDependencies graph. Imports use its bounded pure worklist; source/module absence is never a foreign/prelude/associated absence assertion. Target source/header/body (including body range), declaration context, entry reliability, source member selection and actual immutable plan are still checked by the original producer. Primitive-looking Type aliases/imports remain prerequisites rather than runtime-descriptor recovery.

Exact singleton compatibility deliberately retains the prior privately published namespace/compiler-facet contract; it does not fabricate a source-only graph reconstruction of separately admitted compiler-facet macro imports. Its actual one-source membership, both seals, all source/declaration/compiler/currentness guards and immutable plan checks still execute. Callers cannot force singleton in a larger published captured world because actual membership rejects that omission.

Admission identity domain advances `lrr/v1/current-constant-source-admission` to **v2**, adding exact retained SourceSelection path + full OutlineKey encodings. No stable InputKey/InputValue/query/wire tag changed. Root owns any typed ConstantKey/result cache compatibility consequences. No persistent restoration implementation or acceptance is claimed.

## Checked retained cost and invalidation

<=256 original views; existing <=100k current input stamps/32MiB bounds. Checked per-lease prepared index + per-view metadata/vector charge; conservative combined source-bytes*128 reservation before private graph/parser work. Existing graph import work/budget guards remain. All current input membership/fingerprints are retained, so derived publication/epoch/compile/source changes invalidate even without advancing source_revision. Revalidation remints and compares immutable certificate/plan identity. No guest handles, tickets, mutable guest values, thunk/body-structure forcing, macros/native execution, query providers or input mutation occur.

## Exact remaining prerequisites

This is a conservative whole-current-captured-world source lane, not a computed minimal dependency closure. A scoped subset of a larger world refuses rather than certifying absence. Superseded prepared ScopeMembers, ordinary ModuleRecord frontier retirement, generated/replacement/procedural/native/foreign associated-member source authority and incomplete import/prelude/library facts remain explicit typed integration prerequisites if original replay cannot establish them. The foreign named-path selector remains a separate prerequisite: admitting the actual captured upstream provider FOO plan is not entering root lib::FOO or executing whole main.

The existing macro-proof producer still authorizes exact singleton source/body/range facts. Multi-source panic proof production is not added; reached missing proof remains its existing precise prerequisite. No effects are driven to obtain it. No unused body semantics are evaluated by source admission.

## Acceptance preregistration and evidence

Thirteen attached controls are **uncompiled/unexecuted**. Genuine unchanged cross-crate const-fn root+aux supplies actual two-unit source graph/provider FOO plan all4 editions (expected pure result1); original module BITS source owns inline u8 context/result8 in a multi-source world all4; original Type/Value reexports preserve Value A/result0 all4. Supplemental current provider/set-order; missing/extra/duplicate/wrong/stale source leases; Type/Value source/world mismatch; imported primitive-spelling alias refusal; derived epoch and changed compile facts; exact body/context/edition/cfg; metadata/count budget; unchanged singleton nested original; unestablished imports without provider effects. These are source/plan/evaluator component controls, not full fixtures/main or static diagnostic passes. Genuine ordinary module capture/adoption is not fabricated by supplemental source inputs; its missing join is explicitly retained above.

All originals/aux/directives and both license texts are copied unchanged with ae0fe511b15ab760d7e963b13389d1bc2ab42b18 SHA256 provenance. issue-17718 and cross-crate-array-len source/library/evaluator frontiers remain copied unexecuted witnesses. Installed b940 Rust1.99 sysroot is distinct from the fixture/source pin. Root must use the unchanged-original importer and update shared manifest/ledgers on integration; worker does not mutate them.

Only actual local checks: pinned Rust1.99 rustfmt --check exit0 chunk bdd869; exact frozen-base combined `git apply --check` exit0 chunk206b78; rustc source HEAD read matches ae0fe pin. No Cargo/compiler/guest/target commands or test executions. Run the new `frontend::constant_source_admission::tests::multisource_tests` scope plus affected source/macro/path controls asynchronously on frozen integrated source before acceptance. Potential first compilation/producer API mismatches remain visible pending work; no passing claim.

## Patches

production.patch changes only admission and adds its private child. attachment.patch adds only the new test child to the frozen macro-compatible test helper. acceptance.patch adds its13 controls. originals.patch contains only new pristine upstream files absent at inspection; licenses/pristine copies/provenance retained in originals/. complete.patch is the alternative combined patch; do not apply both combined and individual forms. patch-check/ holds only exact two base files used for textual checking, never a workspace or target.

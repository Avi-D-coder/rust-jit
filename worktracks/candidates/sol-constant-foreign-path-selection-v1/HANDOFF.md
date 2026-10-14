# T054 / WT10-02 exact foreign original constant selection — source-only

A preserved successor based on authoritative canonical fe080d4a; actual base-file hash in baseline.json. Own one existing frontend leaf plus one new acceptance child, and a tiny module attachment. No DB/query/evaluator/wire/memo/runtime/macro/source replacement/target/Cargo/compiler/guest action. Parent owns ledger/claim integration. Prior path/multisource/macro packets are unchanged.

## Confirmed gap and bounded repair

By source inspection, the current selector returns RequiredForeignConstant immediately after any Value definition with a different defining_unit, even when its exact original source is retained, prepared and published. The unchanged run-pass `cross-crate/const-fn-evaluated-cross-crate.rs` main contains `lib::FOO`; its auxiliary has exact `pub const FOO: i32 = foo(1)`. Existing multisource constant-source admission already supports a genuine selected original provider. This packet removes only that premature branch. It does not evaluate foo or FOO.

The normal original namespace/import replay selects the actual definition; the selected foreign member then passes the existing direct original Constant kind, source/header/body/declaration context/owning unit+edition, target certified member, target seal, current dependency/compiler/epoch and matched caller/target world checks. A missing retained target still returns RequiredForeignConstant. Nonconstant Value members remain OtherValueMember; lexical/associated owners remain prerequisites. No source owner/name/descriptor shortcut grants selection.

Additionally, actual published CapturedSource membership must equal the retained original set, not merely the subset named by a scope seal; all selections must be canonical. This is the same conservative exact-current source-set invariant used by the admitted multisource producer. It may conservatively require unrelated captured originals but cannot mint an absence from an incomplete set. Revalidation now directly remints after stamp checks: capture_current rejects an oversized view list before copying source selections, and final identity compares the complete admitted source set.

## Root integration

Apply production.patch (one existing frontend path), acceptance.patch (new frontend/constant_foreign_path_tests.rs), attachment.patch (two-line cfg(test) module), or complete.patch as an alternative. Original files are unchanged; use the existing importer to import the three exact cases listed in upstream-provenance.json. originals.patch is reference material and includes files/licenses that may already exist: do not blindly apply over existing pristine originals.

API is unchanged: ConstantPathSelection::capture_current(db, caller, actualBody, exactRange, retainedViews, byteLimit) returns the existing ConstantPathOutcome; Selected exposes the exact target SourceSelection/definition/request/context/body and revalidate. Root must route the selected target view into the existing multisource admission/typed query bridge. Selection is not evaluation, a constant result, a runtime descriptor/publication proof or executable macro permission. Revalidate before query publication/reuse/materialization using actual current source evidence. Identity domain changes lrr/v1/current-source-constant-path-selection→lrr/v2/... because certificate admissibility now requires complete published source membership and permits exact foreign originals. No persistent wire schema or DB tag changes.

## Acceptance and limits

Twelve new controls are drafts, uncompiled/unexecuted: unchanged cross-crate root+aux selected request and array-length path across2015/18/21/24; mixed caller/provider editions; genuine renamed foreign reexport; same FOO spelling in wrong retained provider; incomplete published source set despite narrower scope seal; missing target Value seal; different world; derived epoch/compile membership; forged published body owner; exact range/byte/count limits and no provider/publication work; reached foreign function remains nonconstant and its invalid cold body unentered. Existing ordinary local/parameter/import precedence and original ambiguous/reexport namespace controls are retained unchanged. No whole original main, length evaluation, const-fn execution, static diagnostic or runtime fixture pass is claimed.

Raw original namespace replay cannot reconstruct separately installed compiler facade/generated/native/prelude authority. Exact equality remains required; those unresolved paths stay a precise integration prerequisite. Generic/associated/uncaptured/generated/foreign-unestablished producers remain bounded typed prerequisites. The check has at most256 actual captured original views, existing100k/32MiB input bounds and conservative checked parser/transient reservation. No future guest body/thunk query occurs.

## Actual static evidence only

- Existing unchanged importer:44650c/exit0,3cases/8files verified at ae0fe511b15ab760d7e963b13389d1bc2ab42b18, both licenses+COPYRIGHT retained.
- Pinned Rust1.99 rustfmt--check:a194b0/exit0,three explicit owned files, skip_children.
- Exact private-base git apply--check: see STATIC-CHECKS.json for actual terminal.

Compilation, asynchronous controls, unfiltered affected banks, strict checks and integrated milestone are parent-owned pending work. No CA/T/H/M completion or interpreter compatibility claim.

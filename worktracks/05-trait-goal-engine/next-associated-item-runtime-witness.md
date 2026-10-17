# Next unchanged runtime witness: generated functions in three owner scopes

October 16, 2026. Bounded selection/handoff only. No production change, fixture import, test, compiler or Cargo execution is authorized or claimed by this record. Root V10 and its actual terminals retain priority. This handoff does not duplicate the existing `macro-nested_stmt_macros.rs` generic ordinary-function follow-up.

## One selected fixture

Use the already imported unchanged `tests/ui/macros/macro-as-fn-body.rs`, 496 bytes, SHA256 `6ae5e64fd603f1817e3e5a8b69c09b7b72cf39ab8ecc8d58df6706c3b9bf3797`, upstream git blob `188c7f7f728c9739868f058639d2f539830e2b01`. Actual pinned checkout HEAD and imported manifest both name `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`. The imported file was compared byte-for-byte to that local pinned file and matches. No import/manifest edit is needed.

Directive: `//@ run-pass`. There are no auxiliary files, revisions, edition directives, compile flags or checked stdout sidecars. Its owning compiletest edition is 2015. All-four-edition LRR route controls would be separately labeled; no all-edition execution has happened here.

The single `def_fn!` macro emits `fn bar()` with an actual block metavariable. It is invoked in trait `Foo` with `println!("foo")`, inherent `impl Qux` with `println!("qux")`, and module scope with `println!("quux")`. An ordinary `impl Foo for Baz {}` supplies the trait-default call target. The complete unchanged main is:

```rust
pub fn main() {
    Baz::bar();
    Qux::bar();
    bar();
}
```

The substantive contract is three different actual source declarations sharing `bar`, selected by nominal owner/trait/default semantics and executed through ordinary call/continuation/cleanup. The source-derived observable order is `foo`, `qux`, `quux`; this is not an invented official stdout golden file. Test the full main and real output effects, never substitute a generated free helper for an associated declaration.

## What this adds and what it does not

The shadowing witness keeps its generic impl method cold and reaches one local function. This fixture reaches generated trait-default and inherent associated bodies as well as a module function. It exercises genuine TraitItems, ImplItems and Items insertion grammars and distinct owning declaration identity. It needs no receiver value construction, dyn dispatch or generic function arguments: each `bar` is a zero-parameter associated/free function. It is separate from the other worker's qualified/generic ordinary module-call witness.

These three bodies are different output bytes, so the original does NOT prove byte-identical repeated expansion handling. Once its common route works, a separately authorized LRR control can repeat one actual `def_fn!` invocation in two supplemental sibling owner scopes and require distinct invocations/parents despite equal emitted bytes. Preserve the official file unchanged, keep that identity control outside imports, and do not claim it passed from the original alone. The existing wrong-parent and equal-metadata/different-Completed-Arc controls likewise remain separate authority evidence.

## Current source findings, not a new runtime terminal

`source_value_goals.rs` still returns typed `ValueGoalDemand::PathAdjustment` when an actual PathExpr has a qualifier or leading `::` (around lines 381–382 at this review). Thus `Baz::bar` and `Qux::bar` do not yet receive ordinary Value-selected associated-call authority from that service. `entered_call_goals::prepare_entered_call` consumes the actual indexed CallExpr and its retained Value selection; it does not establish a general associated-member selection service.

`entered_module_callables::canonical_module_declaration` requires a module owner (`context.scope.owner.is_none()`), and `prepare_entered_module_function` retains a typed `DeclarationOwner` demand for an owned declaration. Do not erase the real associated owner to reuse this proof or use a second printed-name lookup. A common associated declaration receipt must retain actual nominal target, exact impl/trait owner and default/override selection, canonical declaration/body identity, source grammar, profile/edition, hygiene, cfg, output chain, epoch and current selection guard. The same selected body/signature must then project through ordinary SourceCallable and normal call operations.

`GeneratedDeclarations::declaration_grammar` already transports TraitItems/ImplItems, while `admit_attached` retains actual insertion scope and declaration scan. `GeneratedItems::admit` is explicitly Items-only. The new child impl frontier proves recursive item-output/header completion; it does not authorize treating associated output as module Items. Extend actual associated-item attachment through the existing declaration index/source services when demanded, with genuine grammar and parent chain. Do not introduce a macro-specific parser/call evaluator or widen Items admission to bypass ownership.

`traits/header.rs::SourceImplHeader` and `traits/index.rs::PreparedScopeImplHeaders` are borrowed actual requirements/inventory joins, explicitly not applicable-impl or Drop proofs. `traits/mod.rs` still describes a later tabled engine; there is no current `frontend/member.rs`. A scope inventory is not the configured live/facade catalog, and neither header availability nor an empty impl inventory proves `Baz: Foo`, default-body selection or no-Drop. The next common prerequisite is actual associated-member resolution using demanded source Type/owner facts and applicable impl/default selection, with genuine completeness authority. No full static checker or borrow checker is requested.

Historical `2026-10-15-stock-runtime-115.json` records this unchanged fixture as `LRR-E-NAME-MISSING` (reached direct function identity absent before argument evaluation); `2026-10-15-v123-stock-runtime-129.json` also records an interpreter failure. Those remain historical, different sources. No current failure location is inferred from them. Current typed PathAdjustment is source-confirmed; the next actual full-root terminal should determine the immediate runtime frontier.

## Tasks, boundaries and bounded next work

Use actual Section57 numbering: M013 is discovery worklist/seals, M014 nested impl producer scan, M017 symbolic type/binder terms, M018 SourceView propagation, M019 impl/trait headers, M020 canonical goals, M021 conditional impl selection (and M022 tabled recursion where needed). Earlier component prose that called M018 discovery closure was shorthand in error; those source records remain historical and do not alter this ledger or close any task.

The smallest reviewable next implementation claim should target qualified nominal associated-call selection for this fixture through the same actual source/member service, after its required header/catalog facts exist. It must distinguish `Baz::bar` selecting `Foo::bar`'s default body under the real `impl Foo for Baz`, `Qux::bar` selecting the actual inherent body, and unqualified `bar` selecting the module body. Missing completeness/owner/default facts stay typed pending. Do not infer selection from the three type/function spellings, special-case `def_fn`, or implement the fixture with a method whitelist.

Demand only the selected signatures and reached bodies. Actual declaration producer closure remains cold and may inspect headers under Section50; do not prepare all methods or cold executable branches to resolve one associated call. Bind relevant positive/negative member dependencies in the existing pure query/memo framework. Share immutable output/scan/index handles, prepare ancestor indexes once, account handles/metadata with checked sizes and stable encodings. Installed recipes and call entry must validate retained dependencies rather than reparse, rescan the source world or re-resolve a name.

Required future evidence is the full unchanged 2015 main plus precise actual indexed/canonical call targets, three source body entries in order, exactly-once effects, zero assertion-evaluator/call-by-name fallback, correct ordinary continuation/cleanup and source/epoch invalidation. Compare handwritten/generated/wrapper/applicable standard-macro contexts without changing the original. The free/module call's success cannot stand in for either associated call; output equality cannot replace owner/source proof. Associated output grammar, repeated identical outputs, default-body override/ambiguity and all owning editions require explicit controls before claiming that broader feature.

## Source-aware eight-mode pass

1. No assertion/expression evaluator is proposed; use existing indexed call goals, SourceCallable and ordinary operations. Existing MA001/MA007/MA008 debt stays held.
2. Full actual associated function bodies and their block syntax must join ordinary lowering; no literal/break-only generated path (MA004/MA005).
3. Nominal/trait/default member proof must be context-independent; no macro-specific method or numeric inference (MA003/MA006).
4. Preserve actual TraitItems/ImplItems parent, scope and hygiene rather than reject supported generated bodies or relabel them module Items.
5. Signature/header demands do not force unselected executable bodies or another pattern engine; MA002 remains separate.
6. Reuse retained declaration/ancestor indexes and query memoization; no perphase reconstruction or installed-execution planning.
7. Use relevant sealed member dependencies; no full-world namespace construction/comparison or unrelated invalidation on call entry.
8. Retain Arc parser/body/index/source handles and direct locations; no deep immutable BodyIndex clone or repeated whole-file node search.

This is a source-reviewed next-witness proposal, not architecture acceptance. No performance or scaling measurement is claimed. Runtime, full-fixture, compiler-reference, authority-control and tooling outcomes remain separate.

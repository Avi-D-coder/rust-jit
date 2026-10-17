# Selected module callable: bounded common source admission

LOCAL-024, T032/T033 and T043/T045 continuation contracts. This is a source-only
module selection prerequisite under the existing query39/44 migration. It does
not close E-R5-005, MA001–MA008, the interpreter milestone or trait-goal gates.

The unchanged official witnesses are `tests/ui/auto-traits/auto-is-contextual.rs`
(actual module function `auto`, distinct from macro and later local spelling) and
`tests/ui/macros/macro-nested_stmt_macros.rs` (ordinary module generic function
`static_assert_exists` called from main containing genuine nested macro output).
No macro-name or empty-function rule may implement either case. The second also
needs the separately tracked actual explicit generic Type occurrence join.

`RunMachine::select_common_argument_call` already has a current, genuine
`SelectedValueDeclaration` from the same query39 selected Value used by query44.
The Module arm then calls `select_argument_call`, constructs `CallResolver` over
all source roots, resolves caller context and the spelling again, selects an
original/generated callable and checks the resulting digest equals the first
selection. This equality prevents accepting a different result, but disagreement
still causes a second-resolution refusal. `CallResolver::new` checks every source;
`item` scans every source outline until a declaration is found. Reconstructing
that route is unnecessary after a sealed, hygienic selected declaration exists.

The replacement is one cold module-function admission composed inside query44,
then an owned `SourceCallable` projection using current fixed facts and retained
input guards. Query44 and the ordinary lowerer/dispatcher remain the source and
execution entry points; no parser, resolver registry or evaluator is introduced.
Do not call `NameNamespaces::lookup*`, `CallResolver::new/select/module_function`,
or enumerate all sources in this constructor. Do not cast a digest into a receipt.

The selected projection contains `NameDefinition`, target ParseKey, semantic header
digest, exact DeclarationContext and owner None. It lacks the canonical DefKey and
source attachment lease. The actual completed source environment already indexes
scope ownership through `scope_owner`, preserving Missing/Ambiguous instead of
choosing the first source. The common canonical declaration factor must bind the
selected identity and namespace to one actual original outline row or actual
GeneratedSyntax definitions/items row. Original retained source must reuse
`Database::retained_original_declaration` and `RetainedSourceLease`; that checks
actual readiness, anchors, source publication and all exact derived rows. Its cold
outline replay is not permitted at each call entry. Value39 binding classification
and body Type declarations already perform neighboring joins; factoring their
real source authority is a prerequisite, not permission for a second resolver.

Caller and callee source parses need not match. In the nested-statement fixture,
main can use retained syntax containing grault output while the module function
still belongs to original root syntax. Bind the *target's* actual source/header/
context/body, preserving its edition; retain the caller plan only as the real
selection/environment owner. Generated function authority uses the real current
GeneratedSyntax publication and producer seal, not just its DefKey map.

Proposed cold API in `callables::entered_module`:

```rust
prepare_entered_module_function(
    db,
    parent: Arc<PreparedSourceLocalPlan>,
    declaration: &SelectedValueDeclaration,
    selection_guard: Arc<PreparedInputGuard>,
) -> QueryPoll<ModuleFunctionPreparation>
// Ready(Arc<EnteredModuleAdmission>) or Pending(existing typed ValueGoalDemand).
```

The opaque admission is constructed only from the real common canonical receipt,
actual FunctionSignature query and exact published source/header/context/body
locator. It stores the actual guards/leases and checked metadata. Its
`callable_shared` projection returns the same SourceCallable with a private module
receipt and explicit parent retention handle; it never repeats a lookup or parses
source. Existing signature dependency closure must be captured from its genuine
memo too, rather than assuming the Value guard covers later reads. Source/body
metadata is permitted; no child BodyStructure/Thunk or executable arm is demanded.
A missing canonical source, owner, publication, retained anchor or producer remains
typed Pending/NeedInput. Non-functions and bodyless declarations retain their
ordinary unsupported/value distinctions; never guess by a function-like name.

`db::entered_call_thunk` currently attaches the shared guard only for Local;
Module returns to legacy lowering. A constructor alone therefore cannot remove the
legacy route safely. Root must retain the same module selection guard in normal
recipes across phases and in the callable through argument suspension and entry.
Guards validate actual input stamps only; no re-resolution, global revision
comparison, whole-world namespace census or guard refresh is allowed at installed
execution. Legacy namespace checks remain until this integration and evidence land.

Costs: initial scope owner O(log scopes), one owning outline identity scan O(D)
or generated definition lookup O(log G), signature/header/retained preparation
through existing memo/input services. Those cold facts must be retained in query44.
Per entered call, bounded selected metadata clones plus actual relevant guard rows
O(K); no O(all roots) source preflight. BodyIndex/parser/source roots remain Arc
shared. Checked Section32.2 metadata includes the new proof Arc, actual parent
handle, all signature/header/source/context/guard storage and any source lease.
The existing root-accounting DAG must see externally retained parent handles;
report unresolved accounting rather than quietly hiding an Arc inside an Arc.

Removal conditions: replace only the query44 Module double-name-lookup branch and
its phase guard omission once exact source admission is retained and normal call
lowering consumes it. Qualified/associated/function-value/explicit generic paths
retain their typed genuine prerequisites; no local or module missing proof falls
back to a different resolver. Broader historical macro/numeric routes are not
removed by this slice.

Evidence: unchanged complete auto root all four editions and handwritten/user/
wrapper origins; exact module declaration identity, no local/macro substitution;
retained original target/caller parse independence; genuine generated target proof;
no executable child body before reached call; repeated phases/calls use one memo;
stale source/header/context/body/epoch/producer/relevant namespace refusals and
unrelated publication reuse; single-use call/argument effects, cleanup and suspension.
The nested-statement original remains held until its actual Type occurrence/layout
joins also pass. No Cargo/compiler/runtime command is run by this worker.

## Source-only implementation checkpoint

The four-file implementation is frozen in `/private/tmp/lrr-selected-module-admission-v1/COMPONENT.json`. Source fingerprint `b17e1cec19c6a53afc46dc2faf95eae449ca28c130e061da20b7f6dc65dd5202`. It includes the shared canonical module fact delegation in Value39, private guarded admission/projection and five authored source-control functions. The independent review corrected generated identifier/declaration range confusion and charged the new fixed optional handle. No Cargo/compiler/runtime control has executed for this packet. Actual normal Module recipe/dispatcher guard join remains root-owned; the old engine fallback is not removed by this payload. See `worktracks/evidence/2026-10-16-selected-module-admission-source-review.json` for exact all-eight review, source guards, allocations, costs and remaining acceptance.

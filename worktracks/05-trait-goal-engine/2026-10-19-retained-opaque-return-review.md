# M019/M020 retained opaque return source review

Reviewed candidate fingerprint
`a93015d901adcc89c21414237f55bdb9c460e60965fab7798d51e79e04c96d9f`,
parent `e9aef9a7e353eb12fe9f598ba58d247c38234461`, solo LOCAL-049.
This is an intermediate source producer. No opaque semantic, solver-root,
member-dispatch, full runtime or milestone acceptance is inferred.

Shared entry: DeclarationGenerics::prepare fetches its existing immutable
ParseFile/header/context inputs and calls ParsedFile::function_header, factored
from function_signature. OpaqueReturnHeaders::capture walks only the actual
return Type, retaining nested positions, parents, ranges, bounds and impl-token
editions. DeclarationGenerics::opaque_return_syntax resolves in that exact
retained parser. DefinitionReceipt::observe retains the same query Arc and
matches its real source metadata; it does not build a ReadyProvider.

Separate source review against the eight recurring failure modes:

1. No assertion/expression compiler or evaluator is added. Shared Fn header
   admission is factored out of the existing signature route; initializer
   signature handling stays in function_signature. There is no synthetic Fn,
   token-display reparse, guest evaluator or assertion-operand path.
2. Generated declarations use the same DeclarationGenerics query and retained
   ParseFile. Actual normal macro admission supplies their source inputs.
   An unresolved MacroType is an explicit frontier, not an empty opaque set.
   The controls do not execute type macros or establish their expansion support.
3. No method-name inference, numeric policy or receiver-tag selection changes.
   Genuine syntax is not claimed to be an opaque DefinitionId, capture list,
   substituted predicate, hidden type or ParamEnv. Those producers remain open.
4. No original-only semantic exclusion: original, arbitrary emit and wrapper
   declarations join the same route. Source-receipt checks reject mismatched
   snapshots rather than attaching old syntax to new source. Mixed definition/
   caller editions and the full invalidation/expansion-epoch matrix are unrun.
5. No second pattern machinery or executable arm preparation. The return-Type
   walk skips executable blocks, closures, Fn/const/static declarations. New
   controls observe no FunctionSignature/BodyStructure/Thunk/PatternPlan demand;
   the direct-source control additionally excludes SourceLocalPlan. A nested
   declaration inside a const-argument block is not separately exercised here.
6. One cold traversal per existing DeclarationGenerics memo construction.
   Repeated demands retain the actual Arc, and DefinitionReceipt shares it.
   No source planning, query entry, macro execution or IO is added to installed
   operation execution or solver callbacks. No new query key/tag exists.
7. Existing exact source/header/context/parse dependencies govern this memo.
   No world namespace census or installed currentness scan is introduced.
   Whole-source parse changes can already invalidate this metadata receipt;
   this work does not establish selective runtime recipe invalidation or speed.
8. Parser and query inputs are retained by Arc. No BodyIndex deep clone or
   repeated whole-file AST search is added. syntax_at uses retained direct
   source ranges and the existing indexed parser access.

Cold cost is linear in visited return-Type syntax plus retained site/frontier
vectors. Ordinals use checked u32 representation, not an LRR resource cap.
No new accounting, sandbox/static check, unsafe or alternate backend exists.
No measured performance conclusion is claimed.

Remaining alternate routes: source_types::resolve_actual_type still groups
ImplTraitType with DynTraitType and erases generic bounds to a principal trait;
FunctionSignature.result_symbolic still lacks semantic opaque lowering.
Neither route is extended or represented as repaired. The new source receipt
must feed real opaque identity/capture/bound facts, upstream solving and common
member/dictionary dispatch before replacing and deleting those obsolete routes.
Four required compiler clients and full source-fact readiness are still open.

V377/V378 are build-only intermediates. V379 preserves three host-test compile
diagnostics (two generic Debug bounds and one absent inventory API). V380 fixes
the host callers to use genuine prepared value names, with no fabricated IDs.
Exact build/control/runtime outcomes and guards are in the accompanying
V377–V380 evidence. Matching syntax/IR or a passing generic-function main is
not evidence that impl-trait/nesting.rs or actual solver-root execution works.

# M020 compiler canonical-input instantiation — solo source review

Baseline e6f6983b2bbcaa5a0c44c07abb87ebd43ddf3a28; root/LOCAL-049/WT05-02.
Reviewed V359 source fingerprint: `3489c08051803b530412cde18aeb95909601df6c1eba690beba3d9a84cfef935`.
No root/semantic/runtime acceptance follows from source review.

## Actual shared path

Source Q69/Q70 Encoder -> actual Context<PreparedProvider> -> existing
InferenceContext::build_with_canonical/instantiate_canonical -> actual compiler
CanonicalExt::instantiate_projected -> CanonicalInstantiator. Exact selected
compiler owner bodies and complete instantiate.rs folder/visitor are registered
by upstream_canonical_instantiation.py, from licensed pinned originals.
The extension macro is expanded mechanically into the trait declaration and
implementation, preserving method bodies/signatures. New context construction
uses the existing actual Cx and the input's exact TypingMode. No extra mutable
inference owner, allocator, universe mapping or transaction algorithm.

The compiler's highest canonical variable visitor and argument-prefix clause
cache remain. Concrete State owns two pure maps beside its existing erased-type
map; StateRef retains their borrowed view. Keys are actual context handles and
retained argument slices, not text/synthetic source IDs. Lock calls become
single-threaded RefCell borrows. Cache reads copy handles before recursive
folding; Rust2024 if-let temporary scope releases the lookup borrow before else.
No cross-world cache publication, global namespace census or source query.

Mechanical compiler/shared-interface joins: concrete lifetimes/type aliases,
Ty::kind copied instead of borrowed, Args::items slice indexing, explicit stable
visitor Result=(), canonical-kind list passed by Copy handle instead of compiler
reference, (universe,Span) constructors instead of origin-first compiler methods,
float inference omits only compiler lint origin. Original no-HirId behavior is
preserved; no static lint checker is introduced. DUMMY_SP/source diagnostic gaps
from V356 are not relabeled as exact diagnostic parity.

V357's2errors concern explicit stable visitor result and list-reference wrapper.
V358's5errors are host control shapes (INNERMOST spelling and unsupported4-tuple
TypeFoldable). Production source is byte-identical V358->V359. The control uses
shared2/3tuple support, rather than adding a local traversal implementation.
No fixture/directive/auxiliary is edited, skipped or counted as runtime pass.

## Eight recurring modes (not MA audit ID numbers)

1. No expression/assertion compiler/evaluator. Only source IR context, pure
   compiler canonical transport, deterministic tooling and host controls change.
2. No generated literal/break executor. Existing original/emit/wrapper source
   inputs all enter the identical Cx/inference constructor; no macro-name branch.
3. No context-specific member/numeric inference. Canonical kinds drive actual
   upstream fresh type/int/float/region/const/placeholder creation, not a macro or
   expected fixture output. Shared subtype roots are retained exactly.
4. No generated exclusion/fake facts. ReadyProvider remains closed for incomplete
   facts. Canonical placeholder ordinals are IR variables, never source DefIds.
   No opaque source ID or trait root success is manufactured.
5. No duplicate patterns/eager bodies. Source body, PatternPlan and cold function
   preparation paths do not change. Host controls are supplemental IR/ownership
   checks over actual retained source Type/Span, not a replacement language suite.
6. No repeated source preparation. Compiler folds pure retained terms, memoizing
   types and clauses. No parsing, IO, macro expansion, query planning or guest
   execution in callbacks. Highest-clause-variable traversal is cached once per
   immutable Clauses; argument prefixes avoid irrelevant-suffix misses. No speed
   or release scaling claim is made without representative measurements.
7. No full namespace map or unrelated invalidation. Context-local caches contain
   only pure type/clause/argument keys. Existing source dependency/currentness
   guards are unchanged. No portable pointer-based solver answer cache is added.
8. No BodyIndex deep copy/whole-file search. Only existing interner handles and
   inference storage are used; no VM/frame/call-entry immutable body change.

MA004/MA007/MA008 and full architecture/runtime acceptance remain open, as do
actual SolverDelegate/new-solver root/complete relevant ParamEnv/ADT/variance/
source opaque/function binder/member/dictionary/VM joins and obsolete consumer
removal. Legacy compiler canonicalizer/query response modules are not imported
or compiled; the vendored next solver owns its actual canonicalization/search.
No unsafe, sandbox/accounting/custom limits/static checker/live guest rustc
compilation/native/JIT work. Unchanged multidispatch1 remains a runtime failure
at V356 (ambiguous get with staged facade); these new pure operations alone do
not establish repair. Exact commands/terminals/guards will be attached below.

Review follow-up while V359 is frozen: its host ordinary-bound check transports
a standalone ordinary Bound Ty as data. That establishes nonreplacement of that
IR index but is weaker than the intended well-scoped nested-binder contract.
Before promoting that contract, change only the host control to wrap ordinary
bound data in its real Binder, containing both an ordinary bound Ty and a
canonical Ty, then verify ordinary preservation plus canonical replacement under
that Binder. Preserve V359 evidence at its exact fingerprint; it is not evidence
of nested-binder execution. No production algorithm change is called for.

Final V360 reviewed source fingerprint `14b7472ae1ff054ac780307cc817493c167e85db33cac814e0be1e0502f2cb89`; only infer/tests.rs differs from V359, all production source byte-identical. Library6417/control1545 terminal exit0; CLI direct chunk01af24 exit0 (no session).1/1affected source matrix passes including real nested Binder/no escaping ordinary variable, exact input/head/test/CLI guards. V35973714/50477/47049 terminals exit0,3/3controls at original source;2unaffected passes retained there, not a single all3final-source run. V35711783/101(2API errors), V35839757/101(5host control errors) preserved. Audit20089/0 verifies8crates/177files/2587510bytes,0original edits/5existing projections. No actual root/runtime/gate closure; no newer runtime result than V356 ambiguous get. Commands/hashes/terminals: worktracks/evidence/2026-10-19-v357-v360-upstream-canonical-instantiation.json.

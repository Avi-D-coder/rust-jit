# Source-bound inference client — independent solo source review

Baseline d0ae450d4e5a69248984b9616a50a6a3763ac7cb. M020/LOCAL-049/E-R5-014.
Reviewed V341 source fingerprint:
`0ba803ea18450097bf912c32fb84584834e08eeff18a8da0fca991d88f0d4114`.
Validation is pending at this review; outcomes remain separately source-bound.
This is not an InferCtxtLike/SolverDelegate implementation or a runtime feature.

## Actual source and algorithm reuse

`InferenceContext` owns the actual existing Cx, caller-supplied TypingMode,
initially untainted error Cell and the one InferenceStorage. Deref shares that
storage/snapshot owner; it does not clone a transaction engine or interner.
The constructor is called inside the actual PreparedSolverConfiguration source
context. It supplies no source facts, booleans, ready token or solver callback.
No whole-body rust-analyzer/rustc inference or driver is invoked.

`sync_upstream_inference_context.py` registers complete selected operations from
exact originals. Variable creation/span probes and fresh binder replacement use
the pinned compatible RA client. Root-aware shallow resolution and the deep
resolver use the current compiler, not RA's older resolver: the pinned compiler
explicitly requires unresolved equivalence classes to resolve to their roots.
The actual compiler `probe_with_root_vid`/`inlined_probe_with_vid` accessors are
included in the existing TypeVariableTable module so its original private table
and key fields remain private. There is no second union-find or local resolution
algorithm. Constant resolution preserves Alias/fresh/bound/placeholder/value/error
branches. Type-to-numeric chains use the actual recursive compiler behavior.

The compiler deep resolver retains its local DelayedMap, predicate/clause fast
paths, error-taint propagation and full existing TypeSuperFoldable traversal.
Regions remain unchanged by that explicitly named operation. No manual Type
substitution walk or erased/static region fallback is introduced.

Fresh binder replacement uses the already pinned shared BoundVarReplacer and
actual Ty/Region/Const namespaces. It keeps the no-bound fast path. Anonymous
mixed binder controls are supplemental IR controls constructed over real source
Types; they do not establish source function early/late classification or count
as a stock higher-ranked runtime fixture pass.

`generic_args.rs` is the registered pinned GenericArgs::for_item/fill_single
algorithm over LRR's actual retained parent-first flat table. Parent recursion
is represented by those existing typed rows, not reconstructed by name/owner
search. GenericParameter::index is only projection of the actual SourceParameter
index. New var_for_def transports each actual typed namespace to the upstream
fresh-variable constructor. Identity, existing error extension and fresh
construction share this one constructor. The old duplicate identity/error
parameter traversals in constructors.rs are removed. No new parameter order,
default-selection or generic inference rule exists here.

Mechanical API projection replaces compiler/RA TyCtxt/type aliases with the same
Cx/IR family, adds explicit trait imports, maps numeric constructors directly to
existing TyKind/InferTy constructors, preserves current ConstKind::Alias and the
original span/value data, and maps bound-variable delegate generics to the existing
PreparedProvider. Original vendor Rust remains byte-identical. No algorithm is
modified to make a control pass. No original language fixture changes.

## Eight recurring failure modes (separate from MA audit IDs)

1. **Assertion/expression evaluator:** no expression interpreter, AST evaluator,
   VM opcode or assertion handling changes. Shared declaration/Type queries ->
   source_terms Encoder / SourceGenericLayouts -> actual Cx remain the source
   route. New code is the actual inference client over that IR.
2. **Literal/break generated executor:** none added; original, user emit and wrapper
   source Types share Cx and the same variable client. No macro name, output grammar,
   opaque verifier or special runtime payload. MA004/MA007/MA008 wider closure
   remains separate.
3. **Context-specific member/numeric inference:** there is no method selection or
   numeric default/constraint heuristic. Int/float constructors represent values
   already supplied by upstream variable stores. Resolution branches, roots,
   folding and binder substitution are the pinned clients. Handwritten pieces
   own the context and project actual generic metadata only. MA003/MA006 are not
   blanket-closed by this component.
4. **Generated-only refusal:** no new source-origin branch or original-only syntax
   guard. Real declaration spans/editions and source IDs are read from retained
   generic query data. All source kinds use the same generic table and constructor.
   Missing function/binder/type metadata still remains pending in the existing
   source service; no fabricated provider or successful empty callbacks.
5. **Patterns/cold branches:** no pattern machinery or body preparation is touched.
   Existing source trace checks forbid FunctionSignature/BodyStructure/Thunk/
   PatternPlan demands. Binder test IR is explicit supplementary input, not
   fabricated source completion or a second guest pattern implementation.
6. **Repeated planning:** source queries prepare once; the client reads typed table
   rows and interned Types. Fresh variable allocation is necessarily per inference
   demand and is not source planning. The compiler resolver's term folder runs
   on existing IR with its local cache. No installed operation performs parsing,
   Source IO, macro execution or a whole source-plan reconstruction.
7. **World scans/invalidation:** actual constructor/context ownership stays under
   the prepared source factory/world guards. Variable operations use their own
   tables; generic creation performs one relevant definition lookup and linear
   parameter iteration. No namespace census or unrelated recipe invalidation.
   Final answer dependency/currentness/witness integration remains incomplete.
8. **Immutable copies/searches:** mutable cells own execution/inference state;
   Cx, Ty and Args are retained handles. No BodyIndex/source tree deep copy or
   whole-file syntax search is added. Bound folding uses the shared IR visitor,
   not a source walk. Args vectors contain small Copy handles.

No new local unsafe, resource caps/accounting/fuel, sandbox or static compiler
checks. Existing internal metadata index assertions come from the compiler's
constructor; they are not guest resource limits or borrow/privacy/lint checking.

## Evidence scope and remaining integration

New host controls exercise canonical unresolved roots for type/numeric/const
variables, chained type-to-numeric resolution, deep tuple/array replacement while
preserving region variables, repeated resolution, mixed/no-bound binder handling,
real error-token taint, fresh source argument namespaces/spans/order/uniqueness
and shared identity/error construction. They use actual source-backed contexts;
no empty fake provider, guessed declaration identity or default source Type.
Array fold control uses an explicitly typed usize length on the current64-bit
macOS target. The control's internal diagnostic is intentionally issued by Cx
and tests taint transport; it is not an upstream diagnostic-compatibility pass.
Existing exact snapshot/root/world guards remain in the affected source matrix.

Required but still absent: the full InferCtxtLike contract, actual generic
relations/generalizer, forall/placeholder assumptions, visible region/leak checks,
canonical input/response transactions, real delegate/source-fact publication,
parameter DefIds, function/source binder metadata, actual selected member/witness/
continuation/currentness joins and migration/removal of held custom inference.
RA stubbed callbacks are not imported. No panic/default/NoSolution shim fills a
missing interface. Source facts/ReadyProvider/root evaluation remain blocked by
real missing integration, not waived for these host controls.

Cold costs are actual generic-row lookup/iteration, upstream variable operations
and cached IR folding; no new ordinary frame-entry/dispatch preparation. These
are structural observations, not release measurements. Failure/cancellation,
arbitrary source binders, full region/opaque/const/type round trips and complete
runtime/milestone/JIT/Linux/performance acceptance remain unexecuted or incomplete.
Zero new distinct stock runtime gains or refreshed suite total may be inferred.

Final V341 library3993, CLI10628 and controls19038 all terminal exit0.5/5focused controls pass on the reviewed exact source; final source/HEAD/test/CLI guards match. Actual20generic declaration worlds contain48rows/36consts/24early regions; actual140source Types/136guarded TraitRefs span48original/emit/wrapper edition worlds. Existing primary sources are unchanged; source headers/control IR only, original mains unexecuted. Compiler-root/typed resolver/binder/source-fresh checks and old snapshots/ordering/cleanup pass. No production source changed after review. Earlier mechanical failure/build-only cohorts retain their own scope. Full facts/InferCtxtLike/relations/canonical/delegate/root/source-binder/witness/currentness/runtime/milestone/JIT holds remain as above. Exact reproducible commands, sources, failures and terminals are in worktracks/evidence/2026-10-19-v338-v341-upstream-inference-context.json.

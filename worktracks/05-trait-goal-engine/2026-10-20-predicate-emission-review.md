# M019/M020 common transient-bound emission review

Exact reviewed/validated source:
`da7fc5fbb7d486505d275fd78f9b2da67e91bf1d0c52c081f1ae25690488cbab`,
8734 inputs, baseline `7ea94b8f6b3ff5b9a0793a414e2040aa2828d0ff`.
LOCAL-049 / WT05-02; all feature, root, interpreter and JIT gates remain open.

The shared route is `PreparedClauseEmission::prepare` after actual SourceBindings,
then `source_predicates::encode_recipe` and `apply_encoded`. Actual per-definition
Q76 classification supplies Emit, transient positive PointeeSized, or typed
pending. Known non-language classification is distinct from missing input.
The emitter fully constructs the real source predicate and its arguments before
filtering the transient output, matching pinned hir_ty_lowering/mod.rs:983-1002.
Raw/default rows stay retained. There is no comparison with a trait's displayed
name and no proof of the discarded goal. Missing/negative/non-trait/foreign
encoded inputs cannot silently use this positive rule.

All three production consumers join here: PreparedImplSourcePredicates,
PreparedSourceDeclarationClauses (Q80, inherited by Q81), and
PreparedSourceTraitClauses (Q88). The Q84 output-default observation uses the
same emitter; Q82 production publication remains held. Q88 removes only known
transient positive query dependencies; unresolved roles retain conservative
source demands and an explicit whole-query pending result. Q89 follows the
remaining semantic dependencies and runs the same pinned elaborator. No source
lookup is added inside that iterator or installed execution. Actual Q76 roots
and decisions enter the enclosing memo's stable encoding/dependencies. Q76 is
not inserted into SourceBindings, avoiding the Q76->Q75->bindings query cycle.

The separately compiled caller application join retains exact GenericEnv and
SourceCallable Arcs, full actual signature/header/context/layout receipts,
complete zero generic count and direct emptiness of both binding tables.
`source_caller_clauses::encode_applied` and root/no-env `encode` use one private
`encode_empty`; a descriptor alone is not a token. Future root owners must
revalidate exact entered handles and source/currentness before entry/commit.
These new application branches have not executed. No run capture/consumer,
solver root, guest mutation or commit is installed by this checkpoint.

## Eight failure modes, reviewed separately from MA001-MA008 numbering

1. No assertion/expression evaluator. Ordinary source predicate plans use one
   emitter; caller application verifies metadata only (MA001 remains separate).
2. No generated literal/break route, opcode or verifier namespace. Generated
   user Send uses the same Q80 route; ordinary facade core role removal and
   generated user shadow observations are reported separately.
3. No macro/member whitelist or numeric inference policy. This is the pinned
   compiler's authenticated language-item lowering rule; it neither solves nor
   defaults any Type. No custom solver/canonicalizer/retry loop is introduced.
4. No original-only rejection. Actual source IDs, header spans and Q76 receipts
   are shared across editions/origins. Generated core lang-item publication is
   not claimed tested by an ordinary-facade observation.
5. No pattern machinery or cold executable-arm checks. Raw bound/default records
   remain intact; function/method executable bodies stay cold in controls.
6. No source callback from emission/application encoding. Sidecars are retained
   in memo plans; IR transport uses existing immutable context handles. Token
   capture is cold, still awaiting its installed consumer/cache contract.
7. No namespace census or new global revision guard. The existing SourceCallable
   revalidation refusal is inherited and retained, not presented as the final
   relevant-dependency implementation. Sidecars depend on exact catalog/Q76.
8. No immutable BodyIndex clone or whole-file search. Actual direct syntax
   locations and Arc source/query/environment/callable handles are reused.

V522 lib compile failed only at the new test's TextRange API call; no bodies or
later stages ran. Its exact failed fingerprint/old binary observations remain
in evidence. V523 corrects only `.syntax().text_range()`, then all three builds
pass. One actual control body passes in 158.760s with unchanged source inputs:
36 elaboration worlds/36 executions/12 genuine static outlives rows/12 typed
inherent domain holds, plus 12 ordinary-facade transient worlds retaining 24
written rows/filtering 24 transient rows/emitting 0, plus 12 actual user Send
original/emit/wrapper worlds emitting 12 ordinary bounds. Actual source spans,
Self/arguments/binders, default-input provenance, memo/currentness/epoch and
cold executable paths are observed. The owning-2015 repr128 runtime still fails
LRR-E-OP-TYPE with no runtime-check acceptance. There is no new stock gain or
suite refresh. Source/HEAD/test/CLI/digester closing guards all pass.

This establishes the bounded positive-PointeeSized emission component and its
integration into current source transports. It does not establish complete
compiler clauses, generic caller assumptions, root callback closure, response
consumption, activation/commit certification, legacy resolver removal, runtime
compatibility or historical architecture-gate closure.

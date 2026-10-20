# M019/M020 source review — query-fact domains and compiler elaboration

Reviewed source fingerprint
`2dcc5fd54cd54ff3cf2ac812e3b7cd0e7bf4f883414d9a659c7b9300abef2a0c`
(8732 inputs), baseline `da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42`.
V521 validation is asynchronous when this source review is recorded; terminal
results belong in the separate evidence receipt. LOCAL-049 / WT05-02 remains
in progress; this review closes no feature, historical MA finding or gate.

Actual route: Q89 retains Q86 source heads, filtered Q88 clause plans, Q76
language-role receipts and actual configuration/generic layouts. The same
`PreparedProvider::prepare_impl_super_outlives` encodes those inputs, rejects
unavailable callback domains, enters a private immutable scope, and invokes
pinned `rustc_type_ir::elaborate::elaborate` with the exact compiler outlives
filter. Scoped SourceFacts reads use context-owned interned spanned lists and
actual per-definition Sized answers. Scope guards drop before frozen facts can
be installed. This is pure compiler metadata preparation, not a root solver or
another evaluator. The scope cannot set `root_active` or supply an arbitrary
goal. Configuration fingerprints and provider/context identity remain checked.

`ReadyProvider`/replaceable-goal evaluate_prepared have been removed. The
remaining `PreparedIndexSnapshot` describes metadata/index completeness only;
raw root proof-tree entry still requires a private active-root state that has
no constructor yet. The next true root must retain its exact inference owner,
variables, caller ParamEnv, obligations, mode/span and prepared dependency
closure. `root_response::export_root_response` exports original variables from
that same owner; an owned type map itself proves neither solver success nor
runtime commit/currentness. Original variable families come from retained
RootVariables specifications, not a caller-supplied replacement map.

`QueryFact` distinguishes Unprepared, Known(value) and NotApplicable. Genuine
None/false/empty values no longer arise merely from missing neighbors. Trait
flags do not publish empty candidate buckets; positive/negative language-role
queries retain their own known domains. Published associated types validate
actual TypeAlias role/parent even without neighboring full-member enumeration.
Source world/generic receipt, raw/canonical impl membership, complete bucket
coverage, requested leaf/source ancestry and enum repr/schema checks remain.

Q90 and `model::AdtInfo` separate actual nominal kind/discriminant from field,
flag, sizedness, destructor and unsizing queries. Every published variant view
still validates actual source names/order/count; a published flattened field
view requires and validates that variant view. A discriminant-only request does
not force Q74 field Type preparation or fabricate destructor absence. Actual
field encoding reuses Q74/context interners. Missing semantic domains stay held.
The original pinned discriminant client is unchanged.

Caller preparation retains Q75/Q81/Q83/config guards. The authenticated
standalone zero-parameter ordinary-function null branch is distinct from the
explicit-only source hierarchy. Missing GenericEnv does not imply emptiness.
Generic/application/lexical-owner joins remain typed holds. The reached-call
packet in `run_call_inputs` is still inactive; its retained slot/body/call inputs
are not yet an active-call or commit certificate. Existing SourceCallable broad
revision invalidation is inherited debt, not a new permission to weaken guards.

## Eight recurring failure modes

1. No assertion/expression evaluator is introduced. Shared source/trait plans
   and the ordinary call input service are used; the new scopes execute only
   the pinned compiler metadata algorithm (MA001 remains separately numbered).
2. No generated literal/break executor or new opcode/index namespace appears.
   Actual generated declarations use the same Q75/Q88/Q89/Q90 source joins.
3. No method whitelist, macro-selected numeric inference, custom unifier or
   canonicalizer is added. Export preserves actual unresolved numeric families;
   no fallback value or primitive type is invented.
4. No original-only rule is added. The focused observation uses actual original,
   user-macro and wrapper publication in all four editions. This matrix is still
   unexecuted at review time; source inspection does not prove its acceptance.
5. No pattern/control engine changes or cold executable-arm checks. Header and
   optional field Type metadata remain separate from executable bodies. The
   new null caller branch does not infer full clauses for generic callers.
6. No source query in elaboration callbacks or installed execution. Q89/Q90
   retain memo roots; spanned clause lists are interned once per cold context.
   Context construction/encoding remains cold and is not a performance claim.
7. No new dispatch-time namespace census. Published candidate indexing uses
   genuine complete relevant buckets; metadata validation is cold. Caller
   guards inspect exact memo/input dependencies, not all-world comparisons.
8. No BodyIndex deep clone or whole-file node search is added. Parser/query
   roots and source shape/repr proofs use Arc; syntax_at uses retained actual
   source ranges; IR list handles remain scoped to their owning context.

## Explicit remaining semantic hold

The source metadata worker found that common bound emission still reifies an
actual positive PointeeSized bound. Pinned hir_ty_lowering/mod.rs:983-1002
routes that bound's lowered results to a transient vector, and next-solver
assembly/mod.rs:645-646 requires it absent. Existing Q78/Q88 raw/default rows
are valid source provenance, not evidence of complete compiler clause output.
The current Q89 control's raw implied-trait cardinality is not a permanent
semantic assertion. The next change belongs in the common recipe emitter,
authenticated through Q76 after SourceBindings preparation (to avoid the
Q76->Q75->SourceBindings cycle), preserving actual argument preparation and raw
provenance. No spelling filter, builtin-name exception or alternate proof is
allowed. This hold prevents root/factory completion claims regardless of V521.

Actual response/commit consumption, complete generic caller clauses, complete
source-fact factory and root admission, remaining callback domains, runtime
migration and legacy solver removal remain open. Source review and compile/
component observations do not establish an additional upstream runtime pass.

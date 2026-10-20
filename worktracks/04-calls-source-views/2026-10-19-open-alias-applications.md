# M018/T040 — retained open source applications

Solo LOCAL-049, same-root LOCAL-037/044/045/046 interfaces. Baseline
4b525bc40a671cffb757aa4fae78d4376ca8ca2a. Primary unchanged witness:
structs-enums/struct-aliases.rs, all editions, fails at inferred S4 constructor
application after247VMdispatches. No new agent/claim or guest JIT work.

Invariant: keep a generic alias's complete actual target application, including
fixed/repeated/permuted/compound arguments and its real missing declaration
parameters. Do not flatten an alias into unrelated nominal inference holes,
forward caller positions, reinterpret its Params as calling-frame Params, or
infer from a test/macro spelling. Reuse the existing retained TypeAlias /
DeclarationGenerics / AliasNormalizer / mixed Substitution / actual Type walk.
Add one immutable source-application metadata object, not another solver.

Ordinary route: SourceLocalPlan::generic_base_body_type -> selected alias
finalizer -> prepared_constructors::prepare -> ordinary schema/record mapping.
Pattern names use the same finalizer and metadata. BoundRecordConstructor must
retain an open application separately from activation-ready explicit Types;
its binder may retain the target's generic field schema. The source constraint
service reads that same application in record_constructor/open_shape and uses
existing parameter_paths/project_parametric/component work relations to solve
actual alias variables. Fixed arguments and repeated variables remain related.
The existing prepare_open_expression decorates ordinary record lowering with a
retained result hint; no alternate expression or pattern lowering is added.
Installed execution must only consume that retained result/context, never solve
source or materialize an alias parameter as an unrelated caller parameter.

Extend existing source constraints, not upstream trait/canonical/cycle solving.
Missing trait/projection clients still hold the actual upstream root/member/VM
join. Alias const/default/lifetime details retain explicit support boundaries;
no default or region may be guessed. No IO or macro producer side effects in a
query. Source/scope/edition/epoch/query dependencies remain predecessors.

Remove AliasApplication-as-unconditionally-unsupported for admitted Type-only
open aliases, and the pattern alias's unconditional missing-argument refusal.
Keep missing required source facts, unsolved applications, const arguments,
unsupported nominal variant targets and legacy routes explicitly held. Any
nested pattern relation that lacks a genuine projection prefix stays an
explicit inference gap; do not invent a prefix or bypass the common engine.

Preparation is memoized metadata/constraints only: no callee body lowering,
world scan or BodyIndex clone. Runtime checks retained relevant guards/context;
no source preparation in installed execution. Stable domains cover new fields.
No new resource accounting, caps, static checking, backend, unsafe or framework.
Review MA002/MA003/MA006 and all eight recurring modes against exact final bytes.

Evidence: primary original and its actual source metadata all four editions /
original/user macro/wrapper origins, full template argument identities, inferred
result Types, existing constructor/numeric/pattern/cold/exact-once/invalidation
controls, actual unchanged runtime captures. Retain failures. A host/source pass
is not a stock/full-fixture/runtime gate; task/milestone/solver/JIT closure stays
open until its complete contracts are evidenced. Partial/unsupported matrix
entries remain blockers, not convenient exclusions.

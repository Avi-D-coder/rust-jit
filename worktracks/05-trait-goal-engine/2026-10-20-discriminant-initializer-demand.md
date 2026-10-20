# Reached enum discriminant initializer demand

Baseline e53603b0676f689dcfbfe078ebc9a1c886d9595f; solo /root, macOS first.
T040/M018 runtime nominal values, M020 source result integration; E-R5-007/013.
Previous goal turn made progress: pushed source checkpoint and actual unchanged
discriminant_size/discriminant_value runtime captures in every official edition.

Invariant: a reached nonliteral enum discriminant must run its actual anonymous
constant once through the ordinary VM, whether consumed by PrimitiveCast or a
source-selected discriminant intrinsic. Unchanged repr128.rs supplies the
U64Limit/BigNegative expressions and implicit successors. Its complete main
still fails earlier generic operator typing; a prefix-preserving LRR adaptation
outside the original will isolate initializer demand, without counting it as a
stock fixture pass. Keep that whole unchanged runtime failure visible. Existing
unchanged enum const/cast cases remain regression witnesses.

Shared path: actual TypeSchema/header/DefKey -> source-bound variant descriptor
and existing discriminant table -> common source row/anchor -> demand_initializer,
enter_initializer, ordinary source BodyIndex/lowering/VM/cleanup -> const_cells
-> existing agreement-checked table refinement. Source-generated declarations
use the same route; missing generated initializer publication remains an explicit
prerequisite, not permission for another evaluator or original-only new rule.

Extend GuestStore's existing checked reference/aggregate reader and common
pending-table probe. Factor the existing run_enum_discriminants initializer
coordinator for both consumers. CallSeal must return to the ordinary dispatch
loop before sealing when that coordinator demands an initializer; caller slots,
reference leases and call identity survive the existing suspended-call owner.
Use actual LibraryFunction identity selected by the ordinary member service,
never source spelling, macro identity, expected result or a fixture whitelist.

Remove the per-visit filter/Vec reconstruction of PrimitiveCast operands:
InstalledRecipe retains a shared immutable deterministic operation-index/slot
slice computed once at installation. Preparation borrows that retained plan
through an Arc and probes only relevant live runtime operands after resume.
This is ordinary LRIR metadata, not a macro-only namespace or new evaluator.
Installed execution still does no source planning. Cold unresolved discriminants
may demand their existing source row/schema; completed entries only read the
retained table and relevant runtime guards. No namespace census, BodyIndex copy,
new cap, resource accounting, static checker, unsafe or solver is introduced.

Remove duplicated reference tag extraction and duplicated initializer demand
logic; preserve the one scalar reader, source integer identity and wide arena.
An intrinsic's pending value must never be replaced with an ordinal or zero.
Full projected mem::Discriminant fields, upstream solver source/root/member
integration, generated initializer publication, enum/provenance byte images and
the remaining full matrices stay named repair obligations.

Required evidence: actual pre-change failure for the isolated unchanged-source
initializer adaptation, then ordinary VM completion including repeated use and
implicit successors across editions/origins with exact-once observations;
unchanged whole repr128 remains assessed separately. Async macOS1.99 builds,
existing cast/origin/initializer controls, retained-plan identity/resume controls
and closing source/HEAD/binary guards. Record an exact-fingerprint separate
source review against all eight recurring failure modes before checkpointing.
No card, full runtime, full-fixture, interpreter or JIT completion is implied.

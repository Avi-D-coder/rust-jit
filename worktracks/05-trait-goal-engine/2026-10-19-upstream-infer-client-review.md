# M020 upstream inference client — separate solo source review

Root/LOCAL-049/WT05-02; baseline f38017f45c48e9fe05f397e6ba25d5bc22cd84cc.
Reviewed V356 source fingerprint: `27a20003000c9f5918cc30acf1bb7397bd0bff3b0ed7613826abf550449075db`.
This source review establishes route structure, not executed acceptance.

## Actual upstream path and remaining boundaries

Q69/Q70/source Encoder -> retained actual Cx/PreparedProvider -> existing
InferenceContext -> complete rustc_type_ir::InferCtxtLike implementation ->
existing combined inference storage. Full compiler infer/context.rs implementation,
lower_universe/LowerUniverseFolder and entire relate/generalize.rs are selected
from hash-pinned originals. New generalization is the compiler implementation;
it is not an independently written substitute. Shared type_ir SolverRelating
calls that interface and supplies predicate registration in the supplemental
controls. No custom relation is defined to make the controls succeed.

One current inference owner/Type+Const+Region+numeric/opaque/formula state,
combined snapshots, binders and shared fold/relation machinery remain in use.
Legacy local solver candidates stay held and have not gained new consumers.
No real SolverDelegate or root invocation is claimed by this interface compile.
ReadyProvider still refuses incomplete source facts. Real source opaque IDs,
function binders, nominal variance/ADT/member metadata, canonical response and
VM selection integration remain open; empty opaque owner controls cannot prove
nonempty source opaque execution.

The concrete Span/type/import/accessor/lifetime projections are recorded in the
before-code architecture record and deterministic tool. Original source bytes
remain exact and licensed. Original diagnostic TypeVariableOrigin cannot be
reconstructed from the compatible table after subtype unification; the actual
retained group span is used and exact diagnostic fidelity remains open. Known
and Unknown const cases are unchanged. Raw compiler bug invariants still fail;
debug tracing is preserved after fixing the failed substring projection.

Universe/snapshot forwarding explicitly names InferenceStorage where trait
method lookup would otherwise recurse through Deref. No extra transaction
owner or lower-universe rule. No fake source IDs, no source-ready token, no
always-success callbacks, no default no-op region/opaque methods. Standalone
no-Z profile flags use cited compiler defaults via existing retained settings;
Cargo/override profiles remain a gap. TypingMode remains supplied by the caller.

## Eight recurring failure modes, separately from MA001-MA008

1. No expression/assertion evaluator. Changed files are upstream inference,
   retained configuration, source projection tooling and host controls. No
   syntax/thunk/VM/assertion dispatch is added.
2. No generated literal/break route. The existing original/emit/wrapper source
   matrix passes actual handles to the same inference interface across editions.
   No macro identity is consulted by the interface or generalizer.
3. No context-only numeric/member rules. Raw numeric operations use actual
   upstream stores; type/const instantiation uses actual upstream generalization.
   Source trait/member semantics and full runtime inference remain pending.
4. No generated-only exclusion/fake admission. Source-ready guards and source
   IDs stay unchanged. Missing facts cannot be converted into a trait answer.
5. No duplicated patterns/eager executable preparation. No BodyStructure,
   PatternPlan, cold arm or function-body query is introduced. Pure source
   headers supply already prepared metadata; no static checker is added.
6. No parser/planner/query/IO/macro execution in these callbacks. Compiler
   folders/generalizer memoize types; their work is cold inference, not installed
   VM dispatch. New generalizer can demand retained variances via Interner;
   complete ready facts are required before production entry. No speed claim.
7. No namespace/world census or dependency replacement. Existing retained
   interner/provider facts are used; source-dependency publication remains its
   owner's job. No cache or answer is reused across new worlds here.
8. No BodyIndex cloning or whole-file search. Copied Ty/Const/Region/Span handles
   and inference storage are the same existing representation. No frame/entry
   code or immutable body representation changes.

Historical MA004/MA007/MA008 and full architecture/macro/runtime/gate closure are
not implied. Local unsafe is unchanged. No resource budget/sandbox/static borrow/
privacy/lint checking, native guest compilation or early JIT is added.

Required acceptance: frozen macOS library+CLI builds, real-source supplemental
trait-dispatch/universe/snapshot/occurs/placeholder/subtyping controls, deterministic
projection+vendor audit, actual terminal and binary/source/HEAD guards. Failure
cohorts remain failures. Original upstream fixture mains are not run by these
metadata controls; stock runtime gain remains zero unless separately evidenced.

V356 library34609/CLI84716/controls9022 terminals exit0;3/3controls pass with exact source/HEAD/test/CLI guards at the reviewed fingerprint. Real shared SolverRelating invokes compiler generalizer across140source heads/48origin-edition worlds. Projection/vendor audits exit0;8crates/175files/2573921bytes,0original edits/5existing dependency projections. V35377584/101(38errors), V35467818/101(3errors), V35550462/101(6host errors) preserved. Runtime default28278/2 and staged facade22945/2 preserve actual incomplete/fault results; configured witness records31VMdispatches and1call boundary before ambiguous get selection. No new runtime/full-fixture pass or root/gate closure. Commands/terminals/hashes: worktracks/evidence/2026-10-19-v353-v356-upstream-infer-client.json.

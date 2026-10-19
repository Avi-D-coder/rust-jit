# M020 origin-aware obligation state — separate solo source review

Baseline7356f142546db1e51ddd5946c33ed4427942d0c5; root/LOCAL-049/WT05-02.
Reviewed production V351 fingerprint:
`870acfaa7a41825424a0ad37b221fc0228a49705d60ba1b70104cf0b6fbd65d3`.
Reviewed V352 final host-control correction fingerprint:
`9cfe78ba75026a49305c94f8188628700cffea0ef0d650847ce2b90be649d31a`.
Only infer/tests.rs differs; all production sources are identical between them.
Source review is not execution acceptance; terminals attached after completion.

## Actual shared data and entry points

ProvisionalHiddenType is the complete current compiler record's data layout,
actual source Span plus actual Ty. The old span-less OpaqueHiddenType production
name/record is removed, with no compatibility alias or duplicate table. Current
compiler opaque_types/table.rs replaces the older RA table; register/overwrite,
duplicate tracking, removal, count, delta, lookup and drain operations are the
actual pinned bodies. Combined UndoLog::OpaqueTypes stores the whole previous
ProvisionalHiddenType, including origin. No new transaction owner, opaque
selection algorithm, generated-source exclusion or inferred definition identity.

Actual compiler owner methods take/clone/register hidden state and compiler
context lookup/count/delta/duplicate/reset bodies are projected into the same
InferenceContext. They operate on its own storage; they do not return empty
because source facts are unavailable. A newly created owner legitimately has no
registered hidden entries. Nonempty key/lookup/alias callbacks are not executed
without a genuine source opaque-definition producer. Source impl/function IDs
are deliberately not passed off as opaque IDs to make such a control pass.
A direct hidden-record Type/Span check is host data-only, not source opaque use.

Original files remain byte-identical and hash-pinned with licenses. Compiler-only
derive paths are not exported on this concrete record; full opaque Fold/Visit/
canonical round trips remain open. Compiler bug macro projects to a panic with
the same message/failure. No invariant is turned into successful missing facts.
The high-level opaque body-checking/item-bound/borrowck routines are not copied.

TypeOutlivesConstraint now retains the compiler origin field, projected to the
real incoming Span. register_type_outlives_constraint_inner appends that record
and the actual PushTypeOutlivesConstraint entry to the existing combined undo.
The high-level compiler method retains its assumptions-on-binders invariant,
is_global fast path and registration; full ObligationCause diagnostic variants
project to the supplied span. This is explicit diagnostic-data adaptation, not
a change to applicability or an independent outlives/checking engine. Source
registration is not later regionck/borrow checking or a full trait answer.

The exact compiler ty_or_const_infer_var_changed predicate reads the current
readonly type/int/float/const tables, including canonical-root/redirected live-variable wake
behavior. Removed variable handles are invalid, not absent lookup results. Its original inline(always) is retained; no full source/type lookup,
global namespace map, plan construction or defaulting rule enters this path.
Actual docs equate it to shallow resolution changing the term; host controls
check canonical root/unknown/known/rollback/ID reuse states, not guest output.

The shared mechanical client adapter now renames the exact InferCtxt identifier,
not compound names such as InferCtxtUndoLogs. V350 failed5mechanical errors
(compound-name replacement plus missing Region/TypeVisitableExt imports); no
controls ran. V351 fixes namespace/import projection, not semantic rule behavior.

## Eight recurring modes (distinct from MA audit IDs)

1. No assertion/expression engine: Q69/Q70 -> Encoder -> actual Cx -> existing
   inference storage remains the route. VM/expression/assertion code untouched.
2. No generated scalar/break engine: original/emit/wrapper/all-edition source
   contexts use the same records/table/registration/status function; no macro
   name or expansion grammar branch.
3. No context-only method/numeric inference: no member selection/fallback/goal
   algorithm. Numeric wake states read actual compiler stores, not macro identity.
4. No generated-only refusal/fake metadata: source/currentness/dependency guards
   are unchanged and incomplete full facts cannot produce a ready token. No
   invented opaque source key or unobserved source-opaque execution is claimed.
5. No duplicate patterns/eager bodies: no BodyStructure/function body/thunk/
   PatternPlan preparation. IR state controls are supplemental host data, never
   a substitute set of language fixtures. Unentered code stays cold.
6. No repeated planning: source metadata is already prepared; operations use
   retained Type/Span and existing vectors/maps. Explicit opaque clone/delta
   operations have actual upstream cold costs; status checks use readonly tables.
   No source parser/IO/query/macro execution or installed plan reconstruction.
7. No namespace census/unrelated invalidation: no source namespace scan or whole
   world comparison is added. Dependencies remain owned by source preparation;
   mutable state is per inference context and existing snapshot owner.
8. No immutable BodyIndex clone/full-file search: only upstream state/Type handles
   are copied. No frame/call entry or node-resolution path changes.

Separate MA004/MA007/MA008 closure and complete source/runtime/macro cleanup/
suspension/currentness matrices remain open. No unsafe, accounting/cap/sandbox/
static body/borrow/privacy/lint checking/native runtime path or early guest JIT.
Required remaining work: actual opaque-definition ID/classification/arguments,
nonempty opaque/alias/Fold/Visit/round-trip/source origin execution; full source
function binders/flags/InferCtxtLike/generalizer/relations/canonical responses;
full typed source facts/IDs/ADT/ParamEnv/lang-items/currentness; genuine delegate/
root and selected dictionary/member/ordinary VM join plus obsolete consumers'
removal. All original mains in these metadata matrices remain unexecuted. Zero
new stock runtime gain or refreshed suite total; no full M020/milestone/interpreter/
JIT/Linux/performance/diagnostic acceptance follows from this component.

V351 oracle correction: the new host control queried a variable ID after rollback
removed its storage and incorrectly expected a wake result. Pinned ena0.14.4
try_probe_key_value indexes the key before checking root/redirection; absent
indices panic (actual len3/index3 terminal). This is an invalid internal handle,
not valid source/Rust behavior or a production algorithm defect. Preserve V351
failure/source/guards. Do not add an ad hoc bounds check or fake changed result.
V352 checks restored length, actual ID reuse and the new live unknown's status;
all live-root/redirected/known/rollback checks remain. Candidate/query owners must
discard rolled-back handles; end-to-end owner correctness remains pending.

Final V352 library39769/control79434 terminal exit0; CLI direct chunk9d7c89 exit0, no yielded session.1/1affected control passes with exact source/HEAD/test/CLI guards. V351 library27157/CLI91883 exit0; controls37241 exit1 with2untouched passes and explicit host invalid-ID oracle failure. V350 library25622 exit101/5mechanical errors. Only infer/tests.rs differs V351->V352; no production change or test rerun of unaffected controls.140Types/136TraitRefs/48worlds and prior20generic worlds/48rows/36consts/24early regions observed. No source opaque key fabricated or nonempty opaque execution; no root/runtime/gate closure. Exact scopes/errors/commands/hashes/terminals: worktracks/evidence/2026-10-19-v350-v352-upstream-obligation-state.json.

# Common source constraints — LOCAL-024

Pre-code record, October 16. Section 11, T032, M017/M018; MA003 and
E-R5-005. The unchanged upstream
`numbers-arithmetic/integer-literal-suffix-inference.rs` (pinned original SHA256
`f502f13f4f96fe146c05a02dd1a387eb3d4d9932915da35c047975d40eade4a6`)
enters main and fails at `id_i8(j)`, actual terminal `2a3855/1`.
No concrete runtime TypeId was printed. The reached descriptor check is correct.
Source inspection finds no ordinary selected-parameter constraint on the earlier
`let j = -128`; the held runtime-macro inference path cannot supply that fact.

The invariant is that ordinary source constraints, including later source calls,
determine numeric representations before fallback. Macro identity must not select
inference. Source inspection must neither lower nor execute a cold callee/arm.
The repair extends the pinned RA syntax / retained BodyIndex / SourceLocalPlan /
ordinary name selection / FunctionSignature / source Type services and ordinary
Thunk verification. It does not cast a value at the call boundary.

Root owns a common immutable structural source-constraint plan and its existing
pure query-framework integration, then its use in ordinary numeric preparation.
The reference worker owns the shared declared-parameter source goal and genuine
header Type occurrence join, including factoring ordinary argument hints.
The pattern worker reviews the Type boundary without changing root syntax files.
The validator owns frozen asynchronous builds/tests; root does not run Cargo.

First component: `SourceConstraints(BodyKey)` retains a bounded table of actual
indexed syntax facts. Actual body/profile/structural identity joins the source;
direct indexed ranges locate retained syntax, never reparsed display text or a
synthetic body. Facts describe calls, bindings and expression relationships;
they grant no name, type, callable, macro-execution or body-entry authority.
Unknown/unavailable syntax remains an opaque fact, not an eager unsupported
error. A frontend demand must use real hygiene/name/header/Type dependencies.
All immutable facts are charged to the existing memo/query-pin accounting; no
new mutable cache or authority registry is introduced.

Cold cost is one bounded indexed-source pass, direct range/tree-path lookup per
node plus child range lookups; storage is O(nodes + argument/arm edges). Plans
must be memoized once, not rebuilt for each phase. Installed execution must not
fetch this plan or parse source. Relevant solving is a separate cold demand;
unrelated opaque/cold code must not become an executable-shape prerequisite.
Every count/allocation/encoding is checked and explicitly bounded.

Alternate routes remain named and held: `local_numeric_constraints` annotation
and direct-use scanning, `runtime_local_numeric_context`, and especially
`runtime_macro_numeric_context` / `RuntimeMacroNumericAdmission` / the macro
numeric thunk branch. Do not expand their vocabulary. Delete superseded routes
only after the common solver and ordinary lowering consume the same prepared
result across original/user/wrapper/standard expansion contexts.

Required evidence: unchanged full suffix main in all four editions; later typed
uses and shadowing; cold literal-to-source-call branch mutation without callee
thunk preparation; real missing input/suspension; relevant and negative source
dependency invalidation; conflicting numeric constraints; exact-once execution;
bounded retained metadata. Inspect indexed nodes, verifier/dispatch route and
preparation counters. Structural controls supplement upstream originals, never
replace them. All are unexecuted for the new component at this record.

Eight-mode review: (1–4) no alternate evaluator, macro-selected type semantics or
generated-source exclusion; (5) no pattern execution or cold executable-shape
checks; (6) retained memo plan, no installed planning; (7) existing dependency
collector, no world census; (8) owned compact facts, direct node attachment, no
BodyIndex clone. These are design obligations, not passing architecture claims.
Record source fingerprints and actual terminal results before integration.
This intermediate syntax component does not close MA003, M018 or a stock test.

The actual Value occurrence prerequisite uses query39 `EnteredValue(PcKey)`
(entry only). It retains the opaque current source-local plan through existing
LocalPlan/completed-environment retirement accounting, just as query36 does.
Only the owning indexed Path/Pattern and the common hygienic prefix kernel may
mint its private observation; raw metadata remains pending until the real
namespace, declaration-context and binding/constant distinction joins pass.
Its candidate binding target is not binding authority. No callable, callee body
or runtime descriptor can be minted by this query. Root owns query/accounting
hooks; the pattern worker owns its source service and bounded kernel/index
extensions. These staged hooks are uncompiled until that leaf is complete.

After the graph37 two-control pass on `d3ef558c`, a separate successor adds a
bounded derived spelling-to-Path node index. This is solely a candidate filter
for relevant numeric uses: each candidate still needs the real hygienic Value
goal, so same-spelling shadowed locals cannot supply another binding's type.
Raw identifier token spelling shares the filter key; no text is reparsed and
the underlying body/structure identity remains authoritative. The immutable
index is built/accounted once, avoiding a complete fact/AST scan per local
numeric demand. This successor is not covered by the earlier two-control pass.

The next consumer is query40 `BindingConstraints(PcKey)`, entry-keyed and pure.
It joins graph37 with genuine Value39 binding identity and selected local
parameter38 source goals; it never grants callable/body entry. Demand begins
with the reached numeric let and follows the retained candidate-name/reverse-use
indexes. Shadowed uses require the actual binding identity. Parenthesized uses
and ordinary call argument ordinals retain their real indexed nodes. Other
required relationships return a typed pending goal rather than defaulting over
missing source evidence. No selected function body is queried. The module
declaration-environment transport and wider trait/assignment/return constraints
remain explicit prerequisites; no alternate resolver is introduced for them.
Only a resolved concrete numeric source fact may supply the normal local hint;
an unconstrained result alone does not authorize a new fallback path. Wiring
that result into ordinary lowering and installed dependency guards is separate
required work. Until then query40 is an unintegrated source component.

Draft ordinary lowering now selects the retained source hint before the held
legacy local/macro numeric routes. It is not a validated integration. The cold
provider wrapper must attach the real relevant-input closure; installed checks
read stamps only. To avoid recapturing that closure for every let phase, query41
`PreparedBindingPlan(PcKey)` will retain raw goal40 plus its once-captured shared
guard. Recipe clones share the immutable guard and conservatively charge its
payload; guard fingerprinting is cached at private construction. No new cache
registry, parsing, world census or runtime source planning is permitted. The
unsealed historical entry remains an explicitly held route, not architecture
acceptance. Activation evidence must include actual phase reuse/currentness,
not just a source goal's final primitive.

Cold signature dependency correction before numeric activation: the shared
FunctionSignature provider currently reads optional DefinitionBody even though
its header recipe has no body handle. That captures an irrelevant absence in a
future-typed-call constraint; publishing the genuinely reached child body later
could stale the installed let hint. A FunctionSignature must derive from its
actual immutable parsed Fn, source/header/context and declared Type occurrences,
without depending on availability of executable body metadata. Existing
`ParsedFile::function_signature(..., None)` validates that actual header/context
and excludes the syntax body; SourceCallable still separately validates the
real body handle before execution. Preserve TypeAlias body dependencies. This
is a source-reasoned counterexample until a reached-loop witness runs; the
signature-dependency structural control must explicitly inspect the real memo.

Actual candidate compile correction, October 16: frozen numeric V2
`e29bfef4` stopped at terminal `d16aaa/1` (18 errors, zero test bodies).
The failure groups are missing occurrence-module registration, the actual
namespace-certification import, and an omitted opaque proof parameter between
`initial_thunk_admission` and its sole `initial_scope_thunk` call. Exact-baseline
mechanical successors `c2b57a3a` (two leaves) and `67467925` (one leaf) correct
those joins. The latter passes the same borrowed PreparedBindingConstraints;
existing `hint_for` body/node/structure checks and ordinary SetHint/BindLocal
remain authoritative. No parsing, evaluator, dispatch context, type selection
or input dependency behavior changes. All eight recurring failure-mode
conclusions remain limited to these mechanical edits; none closes the held
legacy numeric/macro routes. Full upstream main remains the first execution
required after a successful compile. This is not a runtime pass.

Next bounded compatibility repair: source review `63092721` found the new graph
consumer treats every non-call parent as unresolved, including a cast. The
unchanged, already imported official `auto-traits/auto-is-contextual.rs` contains
`let auto = 10; auto; auto as u8;` after an ordinary unit function call, and is
the primary runtime witness. A cast fixes its result type without equating its
operand's type with that result. Extend only the existing SourceFact::Cast
operand-zero relation in BindingConstraints::prepare; do not propagate the cast
result's annotation into its operand. Actual Value39 binding identity remains
required. This replaces a new blanket Pending response for the known relation,
not a language evaluator. Other unknown relations remain pending. Unconstrained
results still use the explicitly held existing fallback: the new graph does not
yet prove complete absence of constraints hidden in opaque macro output, so
this change must not grant fresh fallback authority or expand that legacy path.

Cold cost adds one fixed branch per visited actual reverse edge; no new source,
namespace, parse, body, index or runtime allocation/authority. Modes1–4/5 retain
one source service and ordinary lowering, modes6–8 retain the existing bounded
indexed cold traversal and installed relevant-input checks. Reproduce the
unchanged official complete main through the production CLI before and after;
any earlier preparation failure remains a gap rather than removing attributes
or the actual user macro. This record is source reasoning, not execution proof.

Required execution-route successor after the ordinary suffix runtime pass:
reuse that same complete unchanged upstream program as the payload of an
arbitrary user macro and a wrapper macro. These are LRR route adaptations
outside the imported originals, not additional stock fixture passes. Factor the
existing all-edition whole-main assertions into one helper, retaining all18 call
and parameter counters, actual nine child plans, exact-once completion and
cleanup/continuation invariants. Do not substitute a smaller language example.
The shared entry is the existing real SourceDiscovery expansion -> completed
source environment -> RunMachine::start_with_discovery -> normal source/query
and VM pipeline. The macro name selects no runtime behavior.

This test-only change grants no source/expansion/type/call authority. It adds no
production parser, evaluator, planning/cache/namespace operation or body copies;
all eight architecture modes remain inspection obligations on the actual route.
Known original-only lexical call gates and primitive-only selected-callee proofs
remain named repair debt; a failing generated execution must drive their common
source-call replacement, never another macro call evaluator. Standard-macro
nested execution remains a separate gap. New executions are unexecuted until
the sole validator collects real terminals on frozen source; earlier original
main evidence is attached only to7406, not this helper refactor.

Pre-execution peer review found uncompiled route-control V1 selected main from
the original outline, which cannot contain a function emitted by a macro.
Preserve V1 as a rejected test setup. V2 uses the existing production
SourceDiscovery::entry_function sealed name/member service and actual
SourceCallable::revalidate to select the genuine body, then the same existing
prepare_source_local_plan/start_with_discovery admission. It does not fabricate
or rewrite the BodyKey. This applies equally to the original helper entry.
The original positive/wrong-argument controls must therefore be rerun against
V2 before claiming unchanged harness acceptance. Both new route executions
remain unexecuted; a setup failure cannot diagnose a runtime call path.

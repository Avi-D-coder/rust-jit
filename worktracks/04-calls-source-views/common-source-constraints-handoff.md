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

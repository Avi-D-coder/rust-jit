# M020 actual nominal declared Type payload

Baseline289995a5accac76fcb386b46d863e1118cbb6e6d. Previous goal turn made
compiled, reviewed and pushed progress (V497); interpreter/root/JIT remain open.
Lead /root owns source_definition_headers.rs and its existing CLI host module;
projection workers own source member identities and TypeIR/binding transport.

Before-code invariant: actual struct/enum/union declared Types are identity ADT
applications of their own real generic parameters. Their Type identity must not
require lowering a body, running an initializer, resolving all fields or
inventing complete DefinitionFacts. Reuse pinned rustc_hir_analysis collect/
type_of.rs nominal branch (196-199), its existing EarlyBinder and the shared
Context::adt_def, Args::identity_for_item, Ty::new_adt constructors. Retain the
exact licensed original and audit the selected branch; this is not the full
type_of provider or a new static type checker.

Shared route: actual DefinitionHeaderPlan/DefinitionReceipt and canonical
source scopes -> SourceBindings/SourceGenericLayouts -> encode_header -> pure
context-owned nominal payload. Original/generated source join before this
conversion. The same key, scope, edition, generic owner and transitive guard
remain mandatory. A nonnominal header or foreign/incomplete generic context
returns a typed pending result. Do not guess function ABI, lifetimes, alias
metadata or semantic absence for other roles.

No alternate evaluator, runtime dispatch or parser is added. There is no new
query: immutable source metadata remains prepared once, and conversion uses
only retained context facts. Cold conversion builds the identity argument list
in O(number of actual generic parameters) and interns the shared Type; installed
VM operations do no source planning. No performance claim, unsafe, count cap,
resource accounting, tool execution or permission policy is introduced.

Unchanged primary witnesses: repr128.rs enum Signed/Unsigned and
union-generic-rpass.rs unions U/MaybeItem. The latter retains a projected field
that may independently need metadata; constructing its nominal Type must not
demand that field. Reuse all editions and actual handwritten/macro/wrapper
header controls, plus metadata cold/memo/withdrawal evidence. Inspect all eight
recurring modes separately at the exact validation fingerprint. Full semantic
DefinitionFacts production, variance/predicates/layouts, complete type_of kinds,
source-header parity, alias/member/root/VM joins and interpreter/JIT gates remain
held. DeclaredType in semantic_holds describes that full factory obligation;
a separately materialized nominal payload does not close it.

## Independent source review

The final V498 source review is bound to
`16c5d5dbee02245bcb283daf850ffcdcf2fbe941291ac05eccda91de2b642b41`
(8,674 guarded inputs; HEAD `289995a5accac76fcb386b46d863e1118cbb6e6d`).
The V499 recheck is bound to
`9f2913e0a577d7a92a3cbdc6c523255901fb3191e65be2425657256a94da0240`
(same HEAD/input count). Only the separate AST text API call and inventory
support-source list changed; the nominal payload, audit and controls are
unchanged. The all-eight-mode nominal review carries to this fingerprint.

Reviewer `/root/wf_validation_review` inspected
`DefinitionHeaderInputs::nominal_type`, the retained header encoding/helper,
the new cold nominal CLI control, and `tools/upstream_nominal_type.py` against
the actual licensed pinned `type_of.rs`. The audit hashes the full pinned
original, extracts its nominal match branch and requires the expected outer
EarlyBinder. After registered representation substitutions it compares the
complete admission wrapper/payload method. The source-only audit returned zero;
build/control terminals are separate evidence, and whole-source fingerprints
are recorded above.
It audits this selected branch, not the remainder of compiler `type_of`.

1. The payload extends retained header conversion; it adds no assertion,
   expression compiler, evaluator or runtime path.
2. `DefinitionHeaderPlan` already joins original/generated declarations before
   conversion. ADT identity is shared regardless of macro origin; no literal
   or scalar-only generated execution is introduced.
3. The actual role, source-world identity and real generic metadata admit the
   construction. No spelling, macro identity, context-only method choice or
   numeric inference chooses a nominal Type.
4. Original and generated sources use the same actual canonical ID and
   `Args::identity_for_item`; no original-only restriction is added. Applicable
   standard-macro and full generated-feature matrices remain unexecuted.
5. The selected compiler payload only constructs ADT identity arguments. It
   does not demand fields, discriminants, initializer execution or unrelated
   arms, and adds no static/borrow checker or pattern machinery. The dedicated
   test explicitly excludes AdtSourceFields/TypeSchema/body/signature/thunk
   demands; its terminal still must be collected.
6. `encode_header` and the context's materialized immutable source generic
   table supply this conversion. No parse, source walk, full-plan reconstruction
   or new query occurs in `nominal_type` or installed execution.
7. There is no namespace scan in the payload, constructor or its guards.
   Existing relevant source dependencies remain owned by the retained header
   plan; no global dispatch census or unrelated hot-path invalidation is added.
8. Query/parser Arcs and actual identity argument lists are reused. The
   conversion is O(actual generic parameters) and adds no BodyIndex copy or
   whole-file node search. No measured performance claim follows.

The source review found no additional alternate route in this delta. Legacy
interpreter member/type preparation and the missing complete factory remain
explicit integration/removal work; neither source inspection nor a selected
nominal metadata control can close those requirements.

V498 terminal: the nominal source audit returned zero, but the library no-run
build failed in the separate associated-type source query (`str_as_str` at
trait_associated_types.rs:147). The bootstrap inventory audit also failed.
No nominal control, CLI build or runtime replay ran. Closing source/HEAD guards
matched the exact V498 fingerprint above; the binary receipt explicitly retains
the old V497 CLI only. Nominal source inspection/audit is component provenance,
not compiled or executed acceptance of this candidate.

V499 build/audit/selected-control evidence: library, CLI and source-digester
builds and all five provenance audits returned zero. The dedicated cold nominal
control passed (59.050 seconds), exercising 24 edition/origin worlds and 48
actual enum/union declared Type payloads with retained identity arguments and
withdrawal. Trace assertions reject AdtSourceFields, TypeSchema, body, signature
and thunk demand. Other new projection controls exposed generated-source gaps;
complete factory/root and broader per-kind holds remain. Remaining controls and
closing source/binary guards completed before returning ownership: the existing
header matrix passed (93.722 seconds; 36 worlds, 276 root and 24 requested facet
headers), and the updated field matrix passed (86.057 seconds; 96 encoded fields,
zero pending declarations). Overall four of six controls passed; the generated
function and generated trait-member source gaps failed in separate projection
selectors. Closing source/HEAD/test/CLI guards matched unchanged V499 inputs.
The unchanged runtime replay remained one generic-derived-type contract pass
and one repr128 `LRR-E-OP-TYPE` interpreter failure. These are separate ordinary
interpreter outcomes, not nominal-factory/root execution or a new stock gain.

The V500 recheck is bound to
`00628ea6f43e24ba37ee37f27a3218381037c26f3ce0087e72774875608180d9`
(8,676 guarded inputs, same HEAD). The nominal payload, audit and dedicated
control are unchanged; common generated Type-name/publication joins and extra
local projection-field/currentness controls changed. Their independent review
is recorded in `2026-10-20-projection-review.md`. Nominal source-route/all-eight
conclusions carry at this fingerprint; V500 execution terminals follow below.

V500 terminal: all builds and five audits passed, with a separate successful
bootstrap `--verify-upstream` local Git-object audit. The cold nominal selector
passed (56.670 seconds, 24 worlds/48 headers); existing header and field matrices
passed. Eight of nine total controls passed; the generated shorthand Tr::T source
field join remained pending in a separate selector, while explicit generated
Trait<usize>::Assoc fields and namespace publication/epoch guards passed. Closing
source/HEAD/test/CLI receipts matched the exact V500 fingerprint unchanged. The
ordinary runtime replay remained generic-derived-type pass/repr128 OP-TYPE
failure. Nominal component provenance/cold identity evidence is established;
complete factory/root/full-type_of and interpreter/JIT holds remain unchanged.

The V501 source recheck is bound to
`0ffc8527d5916f45f60f951c80f40e2cda82cca8060c26b46e3e9b09f9cb461e`
(8,676 inputs, same HEAD). Only common trait-member receipt/consumer paths and
field epoch controls changed from V500. The nominal payload/audit/control remain
unchanged; the source/all-eight review carries, with V501 terminals separate.

V501 is now terminal at that exact fingerprint: library, CLI and source-digester
builds, all five source audits and separate bootstrap `--verify-upstream` were
zero. The unchanged nominal cold selector passed in 55.750 seconds across 24
worlds/48 enum or union headers. All nine selected component controls passed,
including the repaired generated shorthand field route and epoch guards. Final
source/HEAD/test/CLI guards matched unchanged after all processes terminated.
These remain nominal provenance/identity/cold component results. The unchanged
runtime replay was still generic-derived-type runtime-contract pass and repr128
`LRR-E-OP-TYPE` failure; factory/root/full-type_of, full-fixture, interpreter and
JIT holds remain. Exact binary identities and per-selector receipts are recorded
in the companion projection review's V501 terminal section.

# T040/M018 repr and union Type prerequisite — separate source review

Baseline51b35054d37a0c74169150ddc7fe8363d93be7cb. V434 exact source fingerprint
e0364e6ecf8af35ff9c2f3595df24184bcabe9f4285fd57cd1f526399da77532.
MA002/MA003/MA006 are separate audit findings and do not close here.

Inspected run_layout::{declared_repr,declaration_layout_facts,ensure_layout_facts},
runtime/cell_layout::nominal_layout and the one runtime/layout.rs algorithm,
source/manifest::ItemKind::{is_nominal_type,tag}, AliasNormalizer's selected
nominal terminal, source_types::{prepare_nominal,local_nominal_owner},
body_type_declarations's actual declaration/header/name/edition checks,
written_terms::written_type, nominal_constructors::generic_annotation_base,
syntax/type_schemas::ParsedFile::nominal_schema and capture_fields,
runtime/shape's TypeKey::Nominal descriptor admission. Constructor target
structure/local/bind kind checks remain held for union values.

One actual header repr reader now combines packed by minimum and align by
maximum, matching pinned compiler/rustc_middle/src/ty/mod.rs1619-1626,1650-1652.
No expected test value or runtime result is patched. Source type readers now
use the same nominal declaration predicate for Struct/Enum/Union. The union
schema branch delegates to the existing generics/record field capture under
the same immutable source/header match. Runtime descriptor admission uses that
same classification while keeping namespace, argument IDs and all existing
value/arena checks. ItemKind's stable Union tag12 and all old tags are unchanged.

Eight recurring modes:
1. No assertion/expression compiler/evaluator. The original failure at core's
Option::None is the assertion's failure path; fixing header layout does not
introduce another macro execution route. The facade no-hint gap remains named.
2. No literal/break/union-value executor or opaque verifier namespace. New union
code prepares source Type metadata only. No independent struct-like union value
allocation is permitted; overlay/punning remains explicit missing infrastructure.
3. No method-name whitelist or macro-selected numeric inference. Repr keyword
semantics are the actual Rust attribute semantics at one declaration service,
not a fixture/operation/context identity. Type kinds are actual admitted source.
4. Original/generated and module/local nominal Type admission share the same
predicate, owner/kernel, parser/schema and guards. No generated-only exclusion,
guessed scope, copied native type or source identity fabricated from a descriptor.
5. No patterns or eager executable branch/callee preparation. Union field syntax
is a declaration fact; no field initializer or guest body executes in that query.
Cold assert macro operands are deliberately not expanded for the source inspector.
6. No source parser or new AST walker is added; the existing schema parser admits
actual Union syntax and delegates field capture. Repr aggregation stays in its
existing O(attribute token count) cold pass. Source/schema/body Type query memos
retain their actual dependencies. Installed operations do not replan source.
7. No global namespace map/census or unrelated invalidation guard is added.
Actual Type namespace/source/header/schema/edition facts remain required.
8. No BodyIndex/parser deep copy or repeated whole-file search is introduced.
Schema parsing resolves the same retained exact declaration range; Type sites
use retained indexed occurrences. No new resource accounting/limits, static
checker, sandbox, unsafe, frontend/query/native backend choice or trait solver.

V431 builds/3retained layout/pointer controls pass; original advances7578->33281
steps to Union Type selection. V432 source control0sites failure is a bad
inspector filter, retained as failed; existing2controls pass. V432 runtime probes
hit the old nominal descriptor's remaining Union exclusion, retained as invariant
failures. V433 build fails only the host inspector's type-occurrence module path;
V434 uses the actual source_body_context::type_occurrences path. V434 controls and
runtime probes are pending. These older sources are not relabeled current passes.

The source inspector now reads actual indexed RecordConstructor Type occurrences
in the unchanged union-align main and Q73's metadata/guard (not its TypeSyntax-
only use grant). This neither lowers nor authorizes union value construction.
It must verify actual kind/schema, memo/withdrawal and cold guest execution for
all4editions/3origins. Runtime union values, type punning, moves/copy/references,
const/static union storage and cleanup remain blockers. No whole-fixture union,
T040/M018, interpreter/JIT, suite, diagnostic/tooling/Linux/performance closure.

V437 review extension: fingerprint6ef774f770988bdfe0593a8d757b479ab7313717cad699b25f0bac2f12bbd9db.
SchemaNames::from_definition now uses the same nominal predicate as the source
Type and runtime descriptor services, retaining all namespace/crate/module/
header/source checks. Source probing reports a real pending Type Producer in
the prior Value-only completion world. The inspector now uses the already
existing configured_impl_catalog_discovery and its real published producer
completion, as other pure Type controls do. Production admission is not changed
or weakened by this test-world correction; no seal/fact/default is fabricated.
V435's diagnostic formatting failed to compile a non-Debug outcome; V436 uses
the ordinary existing typed diagnostic and retains the Producer failure. No
failed source/control receipt is promoted. V437 both builds pass; source probe
and whole runtime outcomes are pending. The eight-mode source conclusions also
cover the SchemaNames predicate change, with no new parser/solver or execution
work. Runtime union storage/overlay and facade signature hints remain held.

V438/V440 review correction: the previous Type-world completion hypothesis was
insufficient. Direct module lookup selects U16; the pending result is the
body-local LocalTypePrefixSelection::Lookup(NeedInput), before module-tail
selection. Earlier statement macros in the original main have not been expanded
for this preentry inspector, so they can introduce body-local Type names. The
inspector's demand for a Ready body use was incorrect; no production publication
or certificate rule is relaxed. Preserve the former positive body-use matrix as
unmet, not passed by the renamed component test.

The corrected declaration-metadata control uses actual module lookup and the
existing AliasNormalizer/TypeSchema/SchemaNames/nominal descriptor kernel for a
positive result. Separately, the real body-local uses must remain pending and
name an actual captured body macro Producer event; Q73 must return no source
application, with its actual dependency guard. This preserves the common hygienic
prefix and cold source semantics. It does not claim completed body-producer
preparation or union construction. V439 fails compiling the host ready helper's
Debug bounds on opaque SchemaNames/LocalTypePrefix, corrected in V440 by matching
Ready errors and NeedInput directly without requiring those values to be Debug.
V440 fingerprint e17bcf6c414997f14ba05ee869b54b0cb165f259b8a97efe8a94d0ad9ff04286.
Production remains byte-identical to V437; only the host control changed across
V438-V440. All previous runtime receipts stay attached to their own sources.

V441/V442 review extension: failure observation now traverses retained fragment
frontiers through DeclarationFragment::pending_source_evidence, reported by
SourceDiscovery::catalog_pending_source_evidence. It performs no lookup,
publication, macro execution or branch lowering. Cost on this explicit failure
report is O(sum of retained producer counts), plus formatted output; it adds
nothing to normal dispatch. V441 reproduces an unresolved real macro invocation
inside generated hybrid::check_hybrid, retaining the full item stream.

MacroNamespaces::lookup_invocation_segments now selects an already prepared
ancestor prelude using inherited_prelude after ordinary direct/import/macro-use
lookup and implicit_enabled. The exact module's explicit policy still wins;
only same-crate ancestor map keys are probed. No macro-name whitelist or guessed
scope receipt is introduced. Cold cost is at most one map probe per module
ancestor, with existing path key comparisons; no namespace-map census, runtime
recipe guard, source parsing, executable body lowering, plan rebuilding or
immutable body clone. Absolute/qualified paths and pending/inaccessible/
ambiguous direct outcomes return before this fallback. V4424edition control
passes root/nearest inheritance, disabled ancestor, cross-crate absence and
local/pending priority. This is source routing, not runtime behavior proof.

The generated stream now completes discovery and exposes the remaining nominal
Type recipe omission. generated_result_namespace::lookup_type_declaration_using
selects TypeSchema via ItemKind::is_nominal_type, preserving memo-owned actual
schema/header/scope, GeneratedRows and selected occurrence checks. This is the
same classification as original Type admission, removing a generated-only
exclusion (recurring4/MA003); it creates no alternate language path. Recurring
1/2/3/5/6/7/8 conclusions above remain applicable: no evaluators, macro-specific
methods/inference, patterns, eager bodies, installed planning, global scans or
BodyIndex clones. Existing legacy runtime catalog/argument shortcuts in
DeclarationFragment remain named migration debt; this patch does not enlarge
their vocabulary or count their presence as the common-engine gate passing.
Source/body-use/union-value/facade signature-context matrices remain held pending
actual executions. V442union source matrix failed; do not relabel it green.

Final V443 separate review / actual evidence: source fingerprint
c20b5a491ca05be3c15ff6299a55156a4f9dcb3c064c638afcad6cf2fad02c51,
HEAD51b35054d37a0c74169150ddc7fe8363d93be7cb. Relative to V442 the only
source change is generated_result_namespace's common nominal predicate; the
prelude priority pass remains a V442 receipt on identical macro_names.rs bytes.
Both final builds terminal0;3/3selected controls terminal0. Actual metadata
control passes12edition/origin worlds with at least24real pending body uses;
frame-owner and retained explicit-Type controls pass. Former positive body-use
matrix remains unmet, not closed by renamed/negative controls. Actual unchanged
runtime batch terminal1: struct-aliases retains a complete961dispatch/cleanup
pass; align-struct34603steps/90calls/2initializers and union-align210steps/4calls
still fault at union construction, with incomplete captures/no cleanup. No new
stock gain. All source/HEAD/binary closing guards match. These receipts prove
only the named component, not shared union runtime or the full architecture.
The eight-mode source review above applies to this exact final fingerprint;
no further source edits are made before the checkpoint. Full facade signature
hints, union storage/overlay/projections/moves/Copy/references/cleanup and remaining
legacy macro routes stay held. No feature/task/milestone/interpreter/JIT claim.

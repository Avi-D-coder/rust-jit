# T040 / M018 — shared union Type metadata prerequisite

Baseline51b35054d37a0c74169150ddc7fe8363d93be7cb, after the held V431 repr
candidate (source0e5c09fbab611a57a295b6fe1e68bcba4a30f5fd27fe0002e20b71daeacfa6b3).
Primary unchanged witnesses align-struct.rs and union/union-align.rs, both in
fixtures/upstream/rustc-checkout/tests/ui. V431 reaches UnionContainsAlign's
align_of after33281VMdispatches/86source calls and both real initializers, then
AliasNormalizer excludes the Union ItemKind. Subsequent source review finds the
same omission in the body Type declaration/schema and written Type services.

Invariant: Rust structs/enums/unions are nominal Types selected from their real
Type namespace, edition, hygiene, declaration/header/source and schema. Their
storage semantics differ; metadata classification must not fake struct value
construction for unions. Introduce one ItemKind::is_nominal_type predicate and
use it in the existing AliasNormalizer, canonical prepare_nominal, local nominal
owner, body Type declaration admission, written term converter and generic Type
annotation base. The existing schema parser admits actual ast::Union through
the same source/header match, capture_generics and capture_fields kernel used
for other declarations. Keep original/generated and local/module paths joined.
No new parser, type evaluator, solver, macro-specific rule or namespace bypass.

Existing TypeSchema -> SchemaNames -> DeclarationFacts -> cell_layout /
runtime/layout union algorithm is then usable for actual union Type layout
requests. Descriptor and alias representations already retain ItemKind::Union;
do not erase it. Source/header/schema guards and actual owning edition remain
required, and source preparation stays cold/memoized. Costs remain the same
source token/field and Type-tree walks; no new dispatch parsing/plan rebuilding,
whole-world scans, BodyIndex clone, budgets/caps, unsafe or static checks.

Held runtime requirement: unions still need correct shared storage with one
active initialized member, projection/type-punning, Copy/moves/references and
cleanup. The existing struct constructor route remains restricted to its actual
kinds until that common storage mechanism exists. Do not claim union runtime
compatibility from metadata/layout, allocate all union fields as independent
struct storage, special-case field names/fixtures, or skip remaining originals.
This staged prerequisite may be checkpointed only with explicit holds.

Evidence: actual unchanged union declaration/source Type/schema/guard/cold
controls in all4editions and original/emit/wrapper contexts; unchanged whole
runtime attempts retain every later unsupported effect. Keep V430/V431 failures
at their original fingerprints. Review all eight recurring modes separately
from MA002/MA003/MA006. Full T040/M018/union/interpreter/JIT and compatibility
gates stay held until their actual contracts pass.

V432 both builds pass;2/3controls pass. The new source control selected no sites
because it filtered Syntax nodes while the original Type arguments are inside
cold assert macros. Use the two real record Type occurrences already indexed in
ordinary source; do not expand cold macro operands merely for the inspector.
Q73's source metadata (not its TypeSyntax-specific source_for grant) is the
appropriate selected metadata at a RecordConstructor occurrence. Preserve that
failed control, and still require source/guard/kind/cold evidence in all origins.
Whole originals now hit TypeRegistry's remaining Struct|Enum-only nominal-key
admission before layout executes: update that same pure type classification to
ItemKind::is_nominal_type. No value/arena/session/source invariant is removed;
only an actual nominal Union descriptor may now be interned. Keep enum-specific
operations restricted to enums and runtime union values/overlay storage held.

V434 both builds pass;2/3controls pass. Runtime now admits actual Union Type
keys and reaches SchemaNames' remaining Struct|Enum-only canonical declaration
check (align-struct33284dispatches, union-align11). Use the same nominal predicate
there while retaining namespace/crate/module/header/source checks. The new host
source control reaches a pending Q73 result; strengthen its failure report with
the existing typed demand before choosing a source-world/publication repair.
Never fabricate a seal or change a pending result into acceptance. V433's host
module-path compile error and V434 pending-source/control failures remain held.

V436 source-only probe preserves a real Producer demand from the Value-scoped
source world. This Type metadata inspector must use existing configured impl/
Type producer preparation (as the existing Self Type controls do), then publish
its genuine completion. Do not forge Type completion from a Value seal. The
runtime production path is unchanged by that test-world correction. V435 failed
only while formatting the non-Debug outcome in the host diagnostic; V436 reports
the existing typed diagnostic directly. Both attempts remain failed evidence.

V438 proves the completed module graph selects U16 while the body-local Type
prefix returns a producer. The original main has earlier unexpanded statement
macros, which may introduce body-local declarations; module lookup cannot grant
the later unqualified use. The initial host control's Ready expectation was
incorrect. Preserve that failed control and its old acceptance matrix as unmet;
do not replace the producer with the module answer or special-case assert.

Corrected component control has two separate contracts: positive actual module
union declaration -> AliasNormalizer -> TypeSchema -> SchemaNames -> existing
nominal descriptor metadata, and negative actual body-local prefix/Q73 Producer
joined to its real body macro event with no guest thunk/expansion execution.
Rename it to explicitly say declaration metadata. This is not a positive
body-use/union-value/source-producer-completion claim; those remain held. Actual
V437 runtime reaches constructor admission (align-struct34603steps, union-align
210steps), proving the preceding layout queries execute but not the fixtures.
No architecture exception or permission to skip/reclassify those cases is taken.

V440 declaration metadata succeeds for original2015, then generated item-world
preparation deadlocks before the metadata control. Preserve that failed row;
add cold failure diagnostics to DeclarationFragment and SourceDiscovery's
existing catalog_pending_source_evidence to expose the actual pending captured
producer/annotation/scope, without resolving names or mutating discovery.
This extends only failure observation (linear in retained fragment depth), no
normal dispatch/planning cost or authority change. Inspect the resulting actual
producer dependency before changing the common macro source closure. The full
generated stream and unchanged original remain the witnesses; no smaller
generated stream replaces the failing matrix.

V441 diagnostic identifies the actual blocked macro in generated hybrid module:
assert_eq in check_hybrid, with a real Statements producer. Common macro
lookup_invocation_segments consults preludes.get(scope) exactly, whereas source
preparation installs policies only for the original module outlines. Generated
child modules therefore lose the implicit macro prelude already owned by their
crate/parent. This is M011/M018/MA003 source-name parity debt; not an assertion
execution problem. Extend only MacroNamespaces' ordinary unqualified fallback:
choose the nearest prepared ancestor prelude in the same crate after existing
local/import/macro-use resolution and implicit_enabled check. Remove exact-scope
only selection; preserve pending, ambiguous, inaccessible, absolute/qualified
paths and disabled descendant policy. No hardcoded macro/library name or new
scope authority: only an already prepared ancestor policy may supply a unit.
Cost O(module depth * namespace-map lookup), in cold macro selection only; no
full-world scan, new parser/evaluator, installed planning or immutable body copy.
The complete unchanged union-align item stream across original/emit/wrapper and
all editions remains the primary source witness. Add a small namespace-only
regression for nearest override, disabled ancestry, local/pending priority and
cross-crate isolation; it is an LRR routing control, not a substitute stock test.
Positive prepared body-use and union values remain held. Review all8 modes and
record actual terminals/fingerprints before checkpointing.

V442 both builds pass and all4edition namespace priority controls pass. The full
generated stream now completes discovery and reaches actual union field Type
admission. It exposes one remaining Struct|Enum-only classification in
frontend/generated_result_namespace::lookup_type_declaration_using. Select
TypeSchema via the same ItemKind::is_nominal_type predicate; keep exact actual
source/header/scope/current generated rows and selected occurrence checks.
This removes a generated-only exclusion from the common nominal Type recipe,
not an exemption from hygiene. V442 matrix remains failed at its fingerprint.

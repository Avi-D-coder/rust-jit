# M020 immutable ADT variant metadata component

Baseline `43bad0f0e822464533c06be318777d89ab98f11c`, inherited dirty WF client
work retained. Owner `/root/wf_adt_metadata`, delegated by `/root`; only
`model.rs`, ADT publication sections in `prepared.rs`, and this record.

Before-code architecture boundary: the pinned WF visitor in
`compiler/rustc_trait_selection/src/traits/wf.rs` destructures an ADT constant
and pairs its selected variant's actual fields with field values. Existing
`AdtInfo.fields` is flattened and cannot recover enum variant boundaries;
guessing them would violate source identity and the pinned helper contract.
Unchanged `tests/ui/enum-discriminant/repr128.rs` remains the broader runtime
witness; ADT constant/WF upstream and source-route matrices remain unexecuted.

Extend the existing shared `AdtDefinition`/`AdtInfo` immutable model with
`AdtVariant`, `AdtField`, `variants()`, `variant(rustc_abi::VariantIdx)` and the
pinned `FieldDef::ty` early-binder instantiation shape. Actual fields keep
owner schema position rather than invented field/variant DefinitionIds.
`PreparedAdtVariants::prepare` captures the ordinary `TypeSchema` query and
`DefinitionReceipt`; `PreparedProvider::publish` checks their actual nominal
kind, source receipt, names, ordering, counts and the flattened field view.
Both handwritten and expanded declarations already enter `TypeSchema`; this
adds no parser, evaluator, generated-only route or macro policy.

Preparation reads source/schema only through the existing DB and retains an
input guard. Explicit missing source metadata remains a preparation error or
propagated NeedInput, never empty variants, semantic NoSolution or readiness.
Consumers must require the captured guard current before complete fact
publication. Callback reads are immutable O(1) variant selection and the
existing upstream generic substitution for field types. Publication performs
O(variants + fields) consistency checks once, with no namespace census,
BodyIndex copies, executable-body preparation or runtime source reads.

No `AdtInfo`/`AdtFacts` literal producer exists in `crates/lrr/src` at this
baseline. This component authenticates source shape and consistency only;
the real producer must still resolve actual field Type occurrences through
the ordinary source service and encode them before publishing complete
facts. Structural equality against the flattened list is not that proof.
There is no permanent nonempty-field refusal or successful missing-data
fallback. The source producer, transitive currentness, complete provider,
upstream root, member/VM joins, and runtime/interpreter/JIT gates remain held.
The flattened field route remains required by `AdtDef::all_field_tys`, not an
alternate evaluator; publication binds it to the same ordered variant view.

Acceptance plan: parent-owned asynchronous macOS Rust1.99 build, source-aware
positive/negative shape and provenance controls, actual typed producer
integration, then unchanged official runtime witnesses. No Cargo/test command
is authorized for this subagent. A compiling interface alone cannot close M020.

Separate source-review pass, October20, against SHA256
`model.rs=2633afa2354f7f2290758df11a2cf70523264b462933a599343f215c1a3bc799`
and `prepared.rs=97753a581968951e4d2f2a86fae8580f9d2778d7c720382520fb93aaa92024c7`:

1. `AdtField::ty` uses upstream `EarlyBinder::instantiate`; no assertion or
   other expression evaluator is introduced. Existing unrelated audit debt is
   unchanged; no claim about removing MA001--MA008 follows from this slice.
2. `AdtDefinition::variant` reads retained declaration metadata, never literal
   or break-only macro payloads; runtime macro-output execution is unchanged.
3. `PreparedAdtVariants::require_shape` compares actual names and metadata only;
   no method selection or numeric inference policy is added.
4. `DefinitionReceipt::nominal_kind_and_name` uses the retained ordinary parser
   location and accepts each actual nominal item without an origin exclusion.
   Full generated-source/context acceptance remains unexecuted.
5. `PreparedAdtVariants::prepare` demands nominal source/header/schema facts;
   no pattern engine or unselected executable arm/body is prepared.
6. Preparation and schema validation stay cold; `variants`, `variant`, and
   `AdtField::ty` do not parse source or reconstruct a plan. No runtime timing
   or performance result is claimed.
7. `PreparedProvider::publish` adds one ordered traversal over captured variants
   and fields; no global namespace map is built or compared in callbacks.
   The real complete producer must still require source guards current.
8. The schema is retained as `Arc<QueryValue>` and variants/fields as borrowed
   immutable slices. No `BodyIndex` clone or whole-file syntax search is added.

`git diff --check` passed. No build, test, upstream runtime, origin matrix,
source withdrawal, complete provider or solver-root control was executed by
this subagent. Parent validation owns later fingerprints and terminal evidence;
later edits invalidate only the affected source-review conclusions above.

## Read-only producer/currentness follow-up during V490

Rust sources remained frozen; this section records inspection, not another
implementation or validation claim. The smallest next source conversion is a
retained, declaration-keyed pure plan for one demanded nominal owner, using
the existing helper route below. Do not obtain field facts by preparing a
constructor expression or manufacturing a function signature.

1. Retain the actual completed source environment's `names()` and `sources()`
   (`expand/source_discovery/completed_source_environment.rs`) alongside the
   selected catalog/epoch and `PreparedAdtVariants` source receipt. Construct
   `SchemaNames::from_current_database` (`frontend/types.rs`) for the exact
   captured `NominalSchema`. It checks source views against the graph and
   selects the real local/module nominal definition. This is an input
   requirement of cold declaration preparation, not authority to lower the
   declaration's executable bodies.
2. For each actual schema variant, use
   `SchemaNames::source_variant_fields` in
   `frontend/declaration_nominal_types.rs`. It calls `actual_field_type`,
   which verifies unique owner-schema membership, the actual declaration
   range, exact retained Type occurrence/range/tokens, then calls
   `source_types::resolve_actual_type` with
   `WrittenTypeDeclaration::nominal`. This preserves declaration generics,
   source namespace and owning scope. `Option::None` here is unresolved
   preparation and must remain explicitly pending; it cannot become an
   omitted field, unit Type or complete empty vector. `QueryPoll::NeedInput`
   must propagate. Retain every returned source term in variant/field order.
   The existing helper checks each field against all schema fields, making
   the current cold whole-owner route O(F^2); this is not a dispatch cost or a
   measured performance result.
3. Seed `source_bindings::SourceBindings::prepare` with the nominal owner as
   `DefinitionKind::Nominal`, then call `source_bindings::require_type` for
   every resolved field `TyTerm`. Owner seeding is necessary even for an
   owner with only parameter fields: `require_type` does not add a `Param`'s
   declaration. Reuse real `DefinitionReceipt`, `DeclarationGenericTypes`,
   catalog world/local-unit identities and recursive const-parameter Type
   closure. Never synthesize field/variant DefinitionIds. A pending binding
   remains pending.
4. Build `SourceGenericLayouts::from_bindings` from those real plans before
   `PreparedSolverConfiguration::with_source_context`, so `source_parameter`
   and `source_early_region` can answer the owner's generic/region identities
   and parent ordering. Within that exact context, use
   `source_terms::encode_declared(cx, configuration, &bindings.definitions,
   actual_field_term)`. Bind each result with upstream `EarlyBinder::bind`;
   freeze the same ordered values into `AdtVariant.fields` and the flattened
   `AdtInfo.fields`, then validate through `require_shape`. The encoding has
   no DB access; callbacks must only read the frozen result. Scoped owning
   storage for variant/field slices is still required; do not leak them or
   make a second source planner inside `AdtField::ty`.

The existing `PreparedRecordConstructor` fallback in
`frontend/prepared_constructors.rs` demonstrates the actual source helper
call, but its query is keyed by a reached constructor PC, prepares arguments
and retains only one selected variant. It is not complete nominal-header
evidence and must not be reused as the new provider's declaration identity.
Likewise, `NominalMember` preparation binds concrete Type arguments and
currently filters out const/region arguments; it is not the early-bound
uninstantiated field metadata producer.

Currentness boundary: `PreparedAdtVariants::prepare` captures a
`PreparedInputGuard` from the successful current `TypeSchema` memo.
`require_shape` authenticates that captured schema and receipt and compares
typed shape/flattened values, but it neither calls `require_current` nor has
a `Database`. `PreparedProvider::publish` and `ready` also have no database
argument; they cannot silently refresh or verify source stamps. Therefore
the actual owner/coordinator must require the shape guard and the future
field-plan guard current before publication/root entry, then retain relevant
closure versions with any reusable result. `TypeSchema` alone cannot cover
field name resolution, negative/ambiguous lookup, alias selection, generic
Type queries, configuration or the actual source-binding closure.

Place field resolution under the existing pure memo framework with a real
owner/catalog/epoch key, capture its transitive input closure using
`Database::prepare_input_guard`, and compose actual captured closures with
`PreparedInputGuard::join` where needed. `require_current` inspects published
input stamps and caches checked DB revisions; it performs no query fetch.
The new plan must record the existing name service's relevant positive and
negative dependencies. Do not use a whole-world namespace comparison to
replace that evidence. There is currently no such ADT field-plan query,
typed field producer, slice owner or complete-provider call site.

Remaining representation prerequisites are explicit in `source_terms`:
`TyTerm::Function` returns `FunctionMetadata` because runtime terms erase
safety/ABI; `Dyn`, `Opaque` and `Projection` return their corresponding
metadata pending variants. The future producer must retain actual source
metadata and extend the same encoder for these shapes; it must not infer
safe Rust ABI, normalize projections with another solver or accept a
primitive-only subset as complete. `SourceBindings::require_type` currently
collects nominal children, tuples, arrays, slices, references and function
children; richer aliases/projections require their actual dependency join.
These holds and the genuine full producer/root/member/VM joins remain open
even if V490's separate build/source controls pass.

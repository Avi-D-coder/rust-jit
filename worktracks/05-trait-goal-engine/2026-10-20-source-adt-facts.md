# M020 actual source ADT query leaves

Before-code record, LOCAL-049 / WT05-02, baseline
`da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42`. Ownership is only new
`crates/lrr/src/traits/upstream/source_adt_facts.rs` and this record. Lead owns
query registration, context storage, prepared fact slots and root admission.
No build, test, compiler, fixture copying or commit is authorized to this worker.

Invariant and unchanged witnesses: an actual reached `DiscriminantKind`
normalization must obtain its enum discriminant Type from the real declaration,
without inventing unrelated ADT semantics. Pinned
`solve/normalizes_to.rs::consider_builtin_discriminant_kind_candidate` calls the
retained `discriminant.rs::Ty::pinned_discriminant_ty`; that policy reads enum kind
and the declared integer representation, using u8 for concrete non-enum Types.
Existing imported `tests/ui/enum-discriminant/repr128.rs` supplies i128/u128;
`discriminant_size.rs` distinguishes isize default/C representation from physical
layout size. Preparing the declared Type must not evaluate variant initializers.

Common source route: Q75 `PreparedDefinitionHeader` and `DefinitionReceipt`
authenticate actual original/generated declaration identity, scope and edition;
`TypeSchema` supplies the retained nominal shape and shared `declared_repr`;
`PreparedEnumDiscriminant` retains the existing integer policy. Q74
`PreparedAdtSourceFields` uses `source_variant_fields` / actual field Type service,
`SourceBindings`, actual generic layouts and `encode_declared`. Materialization
uses existing context intern pools, not leaked values or manufactured fields.
`PreparedAdtVariants` is retained as the independent publication shape proof.
No parser, evaluator, numeric rule, projection normalizer or solver is added.

Output is independent available query leaves. Kind/name/packed/discriminant do
not wait for field Type conversion. Variants, flattened fields and a struct's
last declared field Type are published together only when Q74 is ready. Genuine
empty structs may have no tail; enums/unions do not claim struct-tail answers.
Destructor, Copy, structural Sized/MetaSized, unsizing and semantic lang/flag
queries remain unprepared. Lead's `QueryFact` split must preserve these domains;
no complete `AdtInfo` or root certificate can be inferred from capture success.

Preparation keys are actual inventory + declaration ID; memo dependencies retain
header/schema/catalog/Q74/configuration roots, receipt inputs and existing enum
and shape guards. Currentness is checked before admitted entry, never by source
planning inside solver callbacks. Encoding validates catalog/configuration and
actual definition identity. Declaration/schema preparation is proportional to
that declaration; optional field preparation and encoding are proportional to
actual field dependencies and retained Type terms. No whole-world scan, body
preparation or per-operation source walk is introduced. Performance unmeasured.

Eight-mode source review scope (separate from numbered MA audit findings):
1. No assertion/expression evaluator; pinned solver discriminant callback only.
2. No literal/break macro executor; Q75/TypeSchema original/generated join.
3. No contextual member/numeric inference; actual repr policy is reused.
4. No origin exclusion; missing shared receipt/conversion remains explicit.
5. No patterns or executable-arm checks; function bodies/initializers stay cold.
6. Immutable memo roots and source-free encoding; no installed planning.
7. Relevant source/query dependencies only; no dispatch namespace census.
8. Arc retained schemas/field plans; no BodyIndex copies or whole-file searches.
MA001/MA002 alternate evaluator/pattern debt is neither extended nor removed by
this metadata slice. Runtime origin/edition/currentness controls are unexecuted;
the lead/validator must attach terminal evidence to the exact frozen source.

Acceptance remains component-only: actual declared discriminant leaves and
ready field leaves may feed the common provider, but missing callback domains,
predicate closure, admitted root lifecycle and unchanged runtime witnesses must
still succeed before M020/runtime/JIT claims. Host abort policy is unchanged.

## Implemented handoff and source review

`SourceAdtFactsKey { inventory, definition, fields }` encodes field preparation
as a distinct demand. Lead registers Q90 `SourceAdtFacts`; the false demand does
not fetch Q74. `PreparedSourceAdtFacts::prepare` retains actual Q75, own source
receipt, schema, Arc shape proof, and (for actual enums) Arc existing repr plan.
`declaration_info` publishes only kind, packed and declared discriminant leaves;
Known(None) is limited to an authenticated non-enum declaration. `encode` checks
both source-world and actual configuration fingerprints plus catalog root, then
materializes Q74 only for the true field demand. Typed field pending remains
available through `fields_pending`; declaration availability is independent.
`source_variants` clones the actual Arc proof, `definition_id` derives actual
crate locality from receipt/catalog, and `generic_layouts` reuses Q74.
`CurrentSourceAdtFacts` captures the complete memo dependency guard; its
`require_current` additionally checks existing repr/shape guards. The plan's
auxiliary guard method is private, since that subset cannot prove currentness
of Q75/catalog/Q74. Encoding has no DB/source callbacks.

Separate source pass inspected every new source call and actual consumers:
`assembly/mod.rs` selects the real lang role; `normalizes_to.rs:979–1033` and
retained `discriminant.rs` own builtin semantics; pinned
`rustc_middle/src/ty/adt.rs::struct_tail_ty` returns the last declared field's
early binder, not a sizedness result. No alternate production routes were added
or deleted. Existing runtime discriminant-value/layout/initializer execution
is unchanged and remains outside this declaration-Type producer. Unknown
PhantomData/ManuallyDrop/unsafe-field/fundamental flags, destructor, Copy,
Sized/MetaSized constraints, unsizing and field-representation queries are named
typed semantic holds; their QueryFact slots remain unprepared. Fields-false
also leaves variants, flat fields and tail unprepared. No skeleton whole ADT
record or provider-completeness claim is made.

Exact module source reviewed:
`13c76cdca870da92ea08afd7d2afb815f86736abeb119f23a28d8f53fe701789`.
Scoped Rust 1.99.0 rustfmt and whitespace checks completed; compilation/tests
were not run by this worker. All eight source observations above apply to that
fingerprint; all edition/origin/runtime/withdrawal executions remain unmeasured.
Upstream policy pin is `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`;
existing licensed raw sources and unchanged fixture inputs were only read.
Witness SHA256: repr128
`66af2eb56be2026a744d2169f8467d11df4c75e8ea21d204f0948db2c8e445bc`;
discriminant_size
`3188f517b24186d08376bd2eac494b4041ddcbb1d11c6de50186be1a1f4d1871`.

## Read-only reached factory closure after source freeze

This is source analysis, not callback trace or executed root evidence.
`assembly/mod.rs::assemble_and_evaluate_candidates` first structurally
normalizes Self. For actual `Signed`/`Unsigned` call arguments,
`normalizes_to.rs::compute_normalizes_to_goal` obtains the actual projection's
trait reference, proves that trait goal, and checks actual associated own args.
`trait_goals.rs:628` supplies the pinned positive DiscriminantKind builtin;
`normalizes_to.rs:979` then obtains i128/u128 through ADT discriminant metadata.
Q75/Q77/Q87 and real generic layouts supply projection declaration/parent/arg
identity; Q76/Q85 supply authenticated role and scalar trait callbacks; Q90
fields=false supplies kind/declared discriminant. No destructor, Copy flag,
field Type closure, enum layout or evaluated variant initializer is needed for
that particular normalization callback. Rigid unresolved Self can instead use
item-self/non-self bounds (`assembly/mod.rs:863–918`); Q82 written/default
recipes alone do not certify those complete compiler domains.

After normalization, `<Signed as DiscriminantKind>::Discriminant: PartialEq<U>`
becomes an integer trait obligation in the same upstream inference owner.
`assembly/mod.rs::assemble_impl_candidates` calls the actual relevant impl
index, and `trait_goals.rs:60–124` requires actual impl head, default/polarity,
generic arguments, `clauses_of(impl)` and `impl_super_outlives(impl)`. Existing
facade `primitive_impls.rs:181,427` really declares PartialEq for i128/u128;
there is no PartialEq primitive builtin candidate to substitute for these.
Q86 supplies real impl scalars/head and exact Impl inferred-outlives domain;
Q78/Q80/Q81 supply retained explicit source own/parent bundles, Q88 filtered
immediate implied/super inputs, and Q89 supplies inspected elaboration inputs.
Parent admitted pinned elaborate/filter must produce actual super-outlives;
Q89 metadata traversal is not that result. The relevant impl index and actual
callback table must retain their catalog/currentness evidence. Actual `main`
caller ParamEnv, source input equalities/coercions, response readback and
same-owner commit remain separately authenticated production integration.

Concrete next correctness dependency: PartialEq's exact pinned header contains
positive `Rhs: PointeeSized` and `Self: PointeeSized`. Current
`frontend/source_predicates.rs::prepare_bound_rows` records these as Implemented;
`upstream/source_predicates.rs::encode_recipe` /
`source_terms.rs::encode_predicate_using` emit a positive TraitClause. Pinned
`hir_ty_lowering/mod.rs:983–1002` instead makes positive authentic PointeeSized
bound lowering transient: resulting clauses are discarded after source argument
processing. `assembly/mod.rs` explicitly treats a PointeeSized goal as
unreachable because compiler lowering removes it. Thus even ready Q88 source
recipes cannot yet be blindly promoted to complete compiler query values.

Smallest common next production change: retain written/default inputs and
actual bound-site provenance, but apply that exact transient-emission policy at
the shared source-to-clause boundary using Q76/Q84 authenticated role identity.
Use the existing optional-clause result; preserve pending unsupported args and
qualifiers, do not filter by spelling or discard source/default evidence.
Compute elaboration dependencies from emitted actual clauses. Then materialize
the real zero-parameter integer impl clause domain and run the already-admitted
pinned Q89 elaborator. No dyn-compatible=true inference, whole trait-world
completion, Copy/sizedness ADT shortcut or dummy WF root is required. These are
necessary source/production changes, not a claim that this is the last runtime
blocker. All eight architecture boundaries above remain in force; no source
edits, builds or tests accompanied this read-only analysis.

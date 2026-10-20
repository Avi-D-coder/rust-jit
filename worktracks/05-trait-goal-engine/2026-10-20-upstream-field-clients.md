# M020 ordinary ADT fields and pinned reflective-field client

Baseline a3e36397f8dd26c0bc5797d7e42908bce0726a26. Previous goal turn made
compiled/validated/pushed progress; complete solver/root/runtime gates remain
open. The unchanged repr128.rs generic operator boundary remains the runtime
witness driving actual source/type/member integration, never a numeric hint.
Generic-derived-type.rs and array-wrapper-struct-ctor.rs supply actual source
field/generic/const metadata controls; imported originals remain unchanged.

Before-code record: flattened AdtInfo.fields cannot provide actual selected
variant fields and the mandatory reflective_field callback still delegates an
unimplemented compiler client. Extend ordinary source TypeSchema -> existing
SchemaNames.source_variant_fields -> SourceBindings/SourceGenericLayouts ->
shared source_terms.encode_declared. Memoize a declaration-keyed AdtSourceFields
plan through the existing pure QueryKey/QueryValue/SyntaxProvider framework
(tag74), retaining exact inventory/source/generic/lookup dependencies and a
currentness guard. Required missing source kinds remain typed preparation
pending, never guessed Types, unit/empty fields or semantic NoSolution.

Shared Context State/StateRef stores own immutable ADT field/variant slices and
integer symbols; interning is cold, borrowed handles stay valid for the source
context and no string is leaked or tuple-index cap imposed. Callbacks do not
parse, prepare bodies, expand macros or walk namespace maps. Field binder
substitution is the already audited compiler FieldDef::ty transport.

Replace Clients.reflective_field with full pinned rustc_middle::ty::AdtDef::
field_representing_type_info. Preserve actual FieldRepresentingType lang-item
identity (no name/shape heuristic), Value u32 ordinal checks, enum variant/name,
ordinary field substitution, tuple index/name and upstream malformed-input
invariants. Upstream solver callers retain Sized/packed/polarity policy. This
client performs no const evaluation and is never a macro-specific route.
Actual lang-item publication, full fact producer and root admission are still
required; structural fact interfaces cannot close those gates.

The const-evaluation worker separately ports the actual compiler algorithm and
keeps required terminal source query outcomes typed/immutable. Reached guest
const/static initializer execution stays in the ordinary VM; preparation-time
values retain the intentional pure evaluator boundary. No source query may
run compiler tools, execute guest code or hide missing input as None.

Costs: cold source plans may traverse actual fields and their generic/name
closure once through memoized dependencies; retained arrays use O(fields +
variants) storage with no resource accounting/cap. Reflective selection uses
constant-time actual ordinal lookup plus shared generic substitution and
O(decimal ordinal digits) canonical symbol formatting/intern lookup; TypeIR
and declaration/source IDs remain unchanged. No measured scaling claim.

Closure evidence: unchanged official source fixtures through source-owned
contexts/all owning editions and applicable origins; retained slice/field
binding/currentness controls; exact original/helper generator hashes; async
macOS1.99 builds; independent all-eight architecture review at final fingerprint.
Nominal/reflection/root/member/VM integration stays held until genuinely
exercised with real complete source facts. Existing legacy runtime resolver is
an explicitly remaining alternate route, not closed by this component.


Integration also registers the new pure query in cli/preentry_loader.rs's
metadata dependency classification and service/expansion.rs's existing
non-accounted query group. No new accounting, budget or trace cap is added.
The first asynchronous build V493 found exactly these two exhaustive-match
additions and one missing shared Ty trait import; its failed terminal and
closing source/HEAD/old-binary receipts remain attached to that source.

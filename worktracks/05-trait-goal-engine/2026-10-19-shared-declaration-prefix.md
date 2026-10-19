# Shared declaration tables/import/prefix kernel — held M019/M020 prerequisite

Owner /root, LOCAL-049, parent 6fe2db1686. Unchanged witness remains
`tests/ui/coherence/coherence-impl-in-fn.rs`: full impl-head demand is 20/32,
twelve owned local Type heads remain pending. No scope guard may be deleted
or replaced with module-only lookup. Full interpreter/JIT goals remain intact.

The existing ordinary route is SourceLocalIndex::lookup_prefix_inner over
EnteredMacroIndex and retained DeclarationFacts; SourceLocalTables stores both
runtime bindings and declaration/import tables. LocalTypeNames supplies actual
item/generic hygiene. HeaderTypeSite supplies declaration-owned Type locations.
The import_routes/named_import_routes/import_lookup/glob_lookup/anonymous_import
algorithms use declaration facts, owning edition/module and immutable tables,
not runtime bindings. Those mechanisms must be shared with the cold scope view.

Extend this route by extracting DeclarationNameTables and a borrowed
DeclarationNameIndex with the same ordinary table builder, import algorithms and
lexical item/import traversal. SourceLocalIndex keeps current source/entry/site
validation, runtime Value-binding selection, macro lookup and generic-owner
handling, then delegates its declaration tail to this shared kernel. Remove the
old in-wrapper algorithm bodies, rather than copy them for a new cold resolver.
A declaration-only caller must still obtain its own genuine captured scope,
source occurrence/hygiene, current completion and namespace dependencies.
This extraction alone grants no cold Type authority and cannot close the failed
head control. The cold client/currentness/memo join remains required next.

Retain one actual declaration table allocation with immutable prepared body
plans. Construction is cold, once per actual plan; index borrowing is O(1),
import/prefix costs remain the existing actual scope/import traversal. Preserve
per-call import memo and existing positive/negative outcomes. No new parsing,
executable body preparation, independent trait solving, runtime planning or
whole-world dispatch scan. No performance claim from source inspection.

Acceptance uses existing unchanged upstream-driven local Type/name/import,
closure/constructor/generic and macro-origin controls plus the complete enabled
32-head control and existing interpreter regression. Cover ambiguous imports,
unknown producer frontiers, opaque item hygiene, named aliases, glob/anonymous
imports and cross-edition source identity through existing controls where
available; mark any unexecuted requirement. No new miniature language suite.
A separate all-eight-mode source review and exact fingerprint are required.
MA007/MA008 are constraints, not the eight recurring mode numbering or closed
findings. Cold original/generated name integration, complete source facts,
upstream inference/delegate/solver invocation and runtime acceptance remain held.

## Cold original declaration client (same kernel)

The extraction builds as V284. Extend LocalTypeNames by factoring its existing
item-occurrence capture/finalization; a captured declaration scope supplies
original-root contexts only after require_original_hygiene. No fake entered
body or BodyIndex is constructed. Retain the immutable declaration-name plan
using PreparedSourceView's existing pure publication memo, keyed by the actual
DeclarationScopePlan fingerprint/source profile/cfg. Pending producers remain
in these syntax-only tables: completion is not a boolean cache input.

The actual Type NameRef must carry ParsedFile::declaration_type_name evidence
for its published local declaration. `type_name_namespace::resolve_declaration_name_at`
may obtain the canonical captured scope from the existing associated-declaration
join and route that exact local use through the shared declaration kernel.
Keep header generics first. Preserve NeedOwnerContext/module-tail holds unless
an actual current lookup completes; do not manufacture module absence, discard
annotation frontiers or widen this into a custom local resolver. Generated
owned origins and completed producer/name-table joins remain explicit next work.

Drive this client with unchanged upstream local Drop and local record/name
fixtures in addition to the original failing 32-head control; distinguish cold
source-name/head evidence from interpreter execution. Ordinary entered lookup
must still use the same extracted item/import kernel and preserve its tests.

Source review hold before cold-client integration: source_root prepares pinned
normalization outside providers. The cold provider must reuse an actual
published original/generated hygiene receipt, not construct another root in
the query or take a raw caller root. A retained pure declaration table is
syntax metadata, not completion; missing preparation needs an observed DB input
so completing it invalidates a cached pending answer. No new raw-root cold
factory is retained in production. The ordinary item-occurrence capture and
finalization are factored and used by the actual entered path; the cold
hygiene/currentness/dependency join remains required implementation work.

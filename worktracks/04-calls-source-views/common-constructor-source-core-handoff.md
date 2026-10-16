# Exact constructor source path and one current-schema binding core

LOCAL-024 WT04 T014/T024/T032/M014/M017/M018. This bounded source-only successor
extends the pre-code common-prepared-constructor-binding record. Exact source3
fingerprint `60073b0af3d65a2a82dc4ba67f8885f039f3e198b686de03ab567e8c4861dba7`
is recorded in `worktracks/evidence/2026-10-15-common-constructor-source-core.json`.
It is separate from context-prefix6 `d38d5f6a...` and all immutable frozen packets.
No Cargo/native execution, commit, imported-original or ledger edits by worker.

Invariant/minimal witness: unchanged official
`tests/ui/regions/regions-infer-borrow-scope.rs` Point constructor and x_coord
must retain actual source owning body/header/hygiene, selected Type/member/schema,
field order and cleanup across original, retained and generated contexts. Existing
unchanged `tests/ui/consts/{const-struct,issue-19244}.rs` supply actual initializer
and field-order witnesses. Current original-only engine owner assumption remains a
known failure; this source component does not report it repaired or replace stock
Box/std macro main with a supplemental identity control.

Shared changes: `ParsedFile::constructor_plan` still validates exact body content,
index/body/profile and cfg, but selects the actual indexed node through
`entered_node_syntax` and operands through retained `BodyIndex::nodes_at_range`.
The former complete body-descendant search and fresh complete-index sort are gone.
`record_constructor_path` no longer reconstructs a whole constructor plan or scans
the parsed file for a convenient path. Its private ConstructorPlan/body/span/profile/
cfg and exact current path tokens are checked; its actual indexed RecordExpr supplies
the path. Generated grammar is not refused by source origin. Generic and unsupported
path forms retain TypeContext; `$crate` is not rewritten from its spelling. Each
actual identifier's token edition/attachment is demanded, and first/final name
ranges are retained as parser DATA, not an opaque namespace authority.

`SchemaNames::{bind_variant_with,bind_record_constructor_with}` are the single
existing variant/record algorithms with an injected current-name demand. Graph
metadata adapters call those same cores. `bind_record_constructor_current` supplies
`type_name_namespace::resolve_declaration_name` using the exact selected schema
owner/header/generics, the same Type authority service used by ordinary named
members/reference pointees. Schema/definition identity, generic refusal, record
shape/rest refusal, written-to-declaration mapping, privacy-before-Type and field
Type-recipe boundaries are unchanged. The existing ordinary frontend constructor
binder calls this current declaration adapter and uses the actual path identifier
range for local hygiene. This is schema-header authority only; it never grants a
constructor BODY occurrence lookup from a declaration header.

Remaining authority/integration: `frontend::constructors::bind_record_inner` and
`engine::run_constructors` still contain original owning-source and caller outline
selection; local index construction and source catalogue scan remain existing debt.
They are NOT relaxed, rewritten to another BodyKey or supplemented by another
binder. The common owning-context pure query must transport/retain an actual
PreparedBodyContext plus cached local prefix and exact BODY Type occurrence→sealed
namespace projection before production switch. SourceLocalIndex/EnteredMacroIndex
currently borrow their parents; an owned retained local plan or occurrence query
answer is required rather than per-operation rebuilding. Root owns that query/input/
accounting integration. Generated selected Type/current-schema support does not
justify bypassing missing caller body occurrence evidence. Imported original names,
Point field spelling and runtime descriptor layout are never special cases.

Costs: exact AST selection is O(tree depth); operand index selection is
O(log(index nodes)+same-range roles) per operand, replacing O(body AST) selection
and O(index nodes log(index nodes)) temporary preparation. Existing exact body
content validation remains cold O(body) work and must be memoized with the source
plan; this component makes no measured speed claim. Record/variant mapping and
field binding remain O(selected fields) cold source work; normal guest execution
uses existing construction/transfer/cleanup, unchanged here. No new persistent
cache or unstable hash domain is introduced. New RecordConstructorPath ranges are
fixed metadata in cold parser data; ConstructorPlan stable encoding is unchanged.
All variable operand/field/token arrays keep checked bounds and allocation.

Eight-mode source review at exact source3 (numbers distinct from MA audit IDs):

1. No assertion compiler/wrapper/evaluator is added. Constructor data/type binding
   only; MA001 remains a separate held architecture repair.
2. No literal/break generated-output decoder/opcode. Actual RecordExpr and ordinary
   ConstructorPlan retained; MA004/MA005/MA007 closure is not claimed.
3. The same SchemaNames core and current declaration Type service choose fields;
   no macro identity, field-name fixture whitelist, method or numeric rule.
   MA003/MA006 are unaffected.
4. Parser data accepts actual attached generated identifiers through the same AST
   path. Production original-owner and exact body Type occurrence gaps remain
   explicit and held; this component is not generated constructor acceptance.
5. No pattern machinery or executable arm/body preparation is introduced. Existing
   unchanged zero-repeat/cold operand controls remain required; MA002/MA008 are not
   closed by constructor metadata.
6. Removed record path full-plan reconstruction; runtime source-plan/current binder
   caching still requires root query integration. No prepared constructor runtime
   switch is installed. Existing original binder rebuilding is named debt, not a
   new accepted execution model.
7. No new world namespace map/census is constructed. Current schema name callback
   uses the shared relevant declaration service. Existing graph adapter/catalogue
   scan stays named until actual occurrence/query replacement; no performance or
   unrelated-invalidation acceptance is inferred.
8. Immutable BodyIndex is borrowed/shared and its retained exact-range index is
   selected. Fullbody descendant search and repeated temporary fullindex sort in
   constructor capture are removed. Current body content hashing remains explicit
   cold work; parser/index source authority is preserved.

External controls: unchanged issue-19244 source binder test now checks the actual
path identifier source range and foreign caller privacy refusal before any current
Type query trace work. Existing stock const initializer/frontier/cold-body and
constructor foreign body/profile/zero-repeat/query controls exercise the changed
shared functions. All remain unexecuted for source3. Actual rustfmt1151f6/0 and
whitespace9b0543/0 are syntax/formatting checks only. Required acceptance is the
sole-validator relevant constructor/header/member banks, currentness/namespace
negatives, actual same-operation handwritten/emitted/wrapper/std traces and eventual
unchanged full fixture runs. Stock Box/library/std and generated body occurrence
joins remain outstanding; no feature, milestone or audit closure is claimed.

# T040/T041/T044/M018 — union views separate source review

Baseline87991f9eb69baa58bc1229270961634793e227d0. Candidate V451 fingerprint
5837a4bc8ed1ece775946a6fb7d8567389ab4645f3763cb5706c3f927be8f4d0.
Source-only conclusions below; runtime validation is pending. MA002/MA003/MA006
are separately numbered audit obligations and are not closed by this component.

Inspected cell_memory::{memory_fields,memory_view_value,make_memory_view,
install_union_memory,commit_union_memory,write_memory_view,read_memory_view,
copy_union_memory,retire_memory_views}; common cell::{state,place_stamp,
place_type,place_object,admit_live_value,read_scalar,initialize,replace_with,
extract_from,preview_intrinsic_place_assignment,aggregate_cleanup_transfer_inner,
discard}; layout::{image_scalar,read_plain_image,write_plain_image,
consume_image_source}; pointer child_offsets/cell_layout; AggregateCopy's
copy_value_layout/begin_copy_destination/finish_copy_destination and constant
template traversal. Also constructor/pattern declaration binders, SchemaNames
record mapping, Frame constructor mapping/finish, RunMachine constructor image
layout preflight and existing ensure_layout_facts/declaration_layout_facts.

1. No alternate expression/assertion evaluator. Memory views decode physical
bytes using image_scalar factored from the existing image decoder. All Rust
syntax still lowers through ordinary indexed source and operations.
2. No literal/macro/union-only Rust executor or verifier namespace. Ordinary
ConstructStart/Field/Finish creates the union owner; selected fields use ordinary
places and the same cell state/reference/Copy/cleanup APIs. Kind8 ownership is
published by the reserved aggregate TransferTicket/set_owner transaction;
scalar-only ticket() remains restricted. V450's mistaken scalar-ticket call is
a recorded failure, never a weakened ownership check.
3. No macro-selected inference/method whitelist. Actual union kind determines
overlapping offsets and direct-field drop semantics. Bound field Types/source
identity remain the existing declaration/member service's responsibility.
Nested targets consult existing needs_drop_type; representability grants neither
Copy nor Drop. Full union Copy retains existing Copy admission before cloning
bytes/init flags, with no inactive-field value reads.
4. Record and pattern metadata now admit actual Union declarations through the
same original/generated nominal Type/schema/source services. Named member goals
retain exact identifier-origin/header/scope/guard checks. No generated exclusion
or fabricated source receipt. Legacy original/generated alternate binders remain
held migration debt; no shortcut vocabulary is added to them.
5. One pattern engine remains. Only declaration/nominal kind admission changes;
record matching, capture modes, guards and cleanup retain the common engine.
Layout demands inspect required Type/field facts before actual image commit,
not unselected executable branches or inactive field initializers. Typed view
metadata may cover all structural fields; it does not execute those fields.
6. No source parser/query/AST plan runs inside a memory load/store or VM commit.
RunMachine resolves required layout facts before ConstructFinish and retains
source specialization currentness; existing query/layout tables supply field
Types. Geometry is computed by the existing layout algorithm, with O(field graph)
metadata allocation per object. A selected owning view read clones only its
reached slice. Type-geometry walks are not claimed persistently optimized.
7. No namespace census/maps or unrelated invalidation is added. Region reads are
object-handle accesses and byte windows; writes do not scan other alias views.
References retain their original cells/source-projection chains and see current
canonical bytes. Source guards use retained source handles/current recipe facts.
8. No BodyIndex/immutable parser copies or whole-file node searches. Frame's
constructor source handle clones retained Arcs; fresh per-value field metadata
and image bytes are physical runtime state. No new budgets/caps/accounting,
static checker, sandbox, unsafe, frontend/query/backend or solver is introduced.

Ordinary source routes removed/changed: Struct-only prepared record admission,
all-fields-present checks for union guards, record-pattern Struct-only metadata,
raw Live-only cell reads at core scalar/place boundaries, nominal-struct pointer
prefix mistaken for an enum tag. Normal aggregate ownership and other constructors
keep their existing checks. View stores preserve address/type identity; initial
region byte masks represent source field writes, while byte-image padding may
choose zero like the existing image kernel.

Unmet requirements: wide scalar/pointer/owned relocation views, enum images,
nested in-place destruction continuation, complete alias/Copy/pattern/const/
cleanup/suspension/currentness matrix, and remaining legacy source routes.
Valid cases hitting these gaps stay failures/unsupported and block JIT. No union
feature, architecture, task, milestone or interpreter acceptance claim follows
from this source review. Run unchanged originals and inspect actual outcomes.

Remaining raw-cell read exceptions found by this separate pass (not claimed
exercised/fixed): cell_layout::referent_discriminant, cell_unsizing::{range_operand,
holds_aggregate,array_element_cells}, and other opaque pointer/container storage
operations still contain physical Live-only probes. They need the common logical
state join where their typed view representation exists, not a macro/context
fallback. Wide/provenance view decoding itself remains explicit unsupported.
The ordinary nested-View destructor path is held at extract_from(cleanup=true),
not silently treated as NoDrop. Union direct-field stores are distinguished by
the actual parent's union kind; nested targets keep existing needs_drop_type
requirements. No claim of the full alias/cleanup/library-operation matrix.

V452 re-review: actual NominalMemberSelection::Named now admits Struct|Union
at the prepared member branch (V451 attempted edit was a no-op). The extended
retained-pointer control executes20worlds successfully; three-original owning
harness has2passes/1numeric-context failure. Source9f6904e1... (V453) additionally
extends constructor_definition's *shared* pattern/source constraint selection to
Union, including the existing local declaration header path. No new inference
rule/solver/constant fallback is introduced. Remaining fields/constraints use
Constructed, field_step and occurrence_constructor_step exactly as Struct.
Unchanged align-struct now passes its union destructure/assertion and reaches
missing Box::as_ref. union-overwrite also completes (first measured here; no
morning comparison). union-drop-assign remains failed at replacement cleanup
owner matching; it is neither passed nor waived. Final terminal/guard collection
and any further source changes need their own fingerprint below.

V453 final both builds/3controls pass:20union source worlds, ordinary pointer
macro/control regression, and image source/window ownership regression. Frozen
source9f6904e1da633ef2257f9809c4691a9ad2b296359ada9334265edbc52cf519ce,
HEAD87991f9; runtime batch terminal1 preserves3passes and2failures. Closing
source/HEAD/binary guard0. V454 candidate source437f3cfe077e62d2090b4f7448e3d740d1fa70615cc203407e87629ba88f5c74
adds an ordinary Box AsRef source impl with the exact pinned method body `self`,
requires incoming ticket ownership before memory writes, and adds a plain union
record/tuple pattern adapter to the existing origin matrix. Only the new memory
module is rustfmt-formatted. No alternate parser, projection type guess, macro
rule, new opcode/solver, static check or installed source planning. Box selection
uses ordinary trait/source/member services and existing deref return coercion;
this is a source facade addition, not a name-triggered runtime operation. The
source owner admission change restores validate-before-write ordering; host fault
injection of that admission is not yet executed and not claimed. V454 tests and
whole originals remain pending until their terminal evidence is collected.

V454 both builds/4controls pass at its fingerprint (24union worlds including
record/tuple destructuring, existing pointer/image/Box regressions). Owning
runtime3/5pass; align-struct now enters actual AsRef then fails declared return
coercion, and nested Drop replacement remains held. V455 lib-only build0, no
runtime/CLI test. The separate source pass caught that probing the new Box path
for every reference could needlessly resolve a str buffer lease. V456 gates the
probe by the actual source descriptor's owned_box identity first; non-Box
references retain their original path. No fabricated Type or relaxed source
admission. The shared coercion allocates from a flat resolved location and
consumes only a transferred mutable reference's record, preserving its real
heap location/ancestor ownership. Return contracts remain checked after the
physical coercion. Full Box-array unsizing/dyn/overloaded chains are not added by
this component and remain explicit matrix gaps. Final validation pending.

Final separate pass at V456 source
`ea9022570a4b35c9010c9ad5726d3ade0fb0062c5f290d5908655a43ceceafbe`,
HEAD87991f9, CLI f166b0568bb01bc8a8ff51cb608ac4db12b57524e49ed4e8da7a5595891fa9d1.
All eight modes above rechecked against the changed call sites. Additional
symbols: binding_constraint_nominal::{constructor_definition,field_step,
occurrence_constructor_step}; alloc::boxed AsRef::as_ref; runtime
boxes::box_reference_place, unsizing::{unsizing_coercion,coerce_unsized_value};
frame::{preflight_result_type_coercing,write_unsizing,write}. The reference
coercion uses only the retained runtime Type/owned_box identity and real resolved
place chain. It is shared by typed storage/call/return boundaries, not selected
by AsRef, an assertion or source spelling. Non-Box references avoid the extra
place walk. Its admission/execution repeat runtime lease validation O(Box chain
and enclosing depth), never source planning/global maps. No performance claim.
The earlier eight-mode conclusions remain source-only statements with the named
exceptions; no matrix result is inferred for unexecuted wide/provenance/enum or
nested destructor paths.

Terminal evidence: V454 both builds/4controls0,24union worlds; V456 both builds
and affected Box control0. V456 supplemental unchanged owning2015 union derive,
macro, const-codegen and refutability batch0:83/14/120/167matched operations,
2/0/0/2source calls,0/0/1/0initializers, unit/closed empty streams/cleanup each.
V453/V454 transmute/align/overwrite main passes remain at their own fingerprints.
All seven are runtime-contract receipts, not a refreshed suite count or full
architecture/fixture/diagnostic/tooling gate. Current align-struct fails later
at large-array nominal context after51559operations/156calls; IO dyn witness
fails unimplemented Write::write_all. Nested union Drop assignment still fails
cleanup owner matching at211operations. Every failure stays outstanding.
V456 closing supplemental source/HEAD/binary guard0; a duplicate after-runtime
phase write was refused without overwriting prior evidence, then independent
post-copy comparison succeeded. No source changed after final validation.

Remaining alternate routes and holds are those listed above; this checkpoint
does not remove all legacy nominal/signature/member paths, supply the complete
upstream solver root, or close MA002/MA003/MA006. Full union Copy/drop/cleanup,
source epoch/suspension/invalidations and macro pattern origin matrix require
further unchanged witnesses and route controls. Keep the feature/task/JIT gates
held. This is a reviewed partial implementation checkpoint, not completed repair.

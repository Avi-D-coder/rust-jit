# T040/T041/T044/M018 — shared union memory and typed cell views

Baseline87991f9eb69baa58bc1229270961634793e227d0; LOCAL049 solo owner.
Read WT06, claims, E005/E006/E007 and spec12/13/57. Primary unchanged witnesses:
union/union-transmute.rs (overlapping tuple/u16 and u32/f32 read/write),
union/union-align.rs and structs-enums/align-struct.rs (nominal field/refs/patterns).
They remain runtime blockers, not inapplicable or partial passes.

Invariant: one canonical byte/init region per union object. Field cells are
stable typed views over it, never independent owning values or active-field
switches. Existing references observe later writes through any overlapping view.
Build retained view metadata from actual prepared descriptor/layout fields;
never prepare/execute inactive field initializers. Primitive image decoding is
factored out of existing read_plain_image, not duplicated in a union evaluator.
CellState::View stores region owner, byte offset, concrete Type and optional
structural-view object. Common state/scalar/place/reference reads decode that
view; by-value ownership reads reconstruct ordinary values through the existing
image/construction kernel. Ordinary cells retain their existing representation.
Writes use the same image encoder into the canonical region, preserving bytes
outside the written range. Initialized byte masks reject undefined reads rather
than inventing independent default field values; no static/borrow checker added.

Shared entry points: source ItemKind/constructors and SchemaNames record mapping;
ordinary Op::ConstructStart/Field/Finish and Frame constructor mapping; reached
layout preparation through RunMachine::ensure_layout_facts before image commit;
GuestStore::commit_construction/initialize/replace/admit_owning_read/read_scalar,
resolve_place/reference validation; aggregate Copy/constant template worker;
ordinary cleanup field transfer/discard and pointer child offsets. A selected
union field has its actual source schema identity and Type, same in handwritten
and generated nodes. No parser/evaluator/solver/macro-specific opcode or fake
scope receipt. Existing Type-only union metadata paths join ordinary execution.

Remove Struct-only record admission and all-fields-initialized construction
assumptions only where source kind is Union. Union destruction must not schedule
Drop for its field views; explicit union Drop itself remains ordinary source
member dispatch. Retire only physical view metadata when the union owner dies.
Whole union Copy clones canonical bytes/init state and rebuilds fresh view
metadata through the existing aggregate-copy continuation, never reading every
inactive field. Moves transfer the single real object owner. A private constant
template uses the same physical image copy without granting Copy.

No new LRR budgets/limits/accounting/sandbox/unsafe. Physical view allocation is
O(actual structural field graph); scalar view reads are O(lease chain + scalar
bytes), writes O(reached image size), without scanning alias views. Preparation
uses retained source/type/layout facts, no dispatch parser, global namespace
census, BodyIndex copy or unselected executable branch. Keep relevant source/
epoch/member checks and exact constructor/cleanup ownership on suspension.

Integration may be staged only with explicit holds. Existing image primitives
support structural plain data; wide scalar view handles and pointer/owned
relocations require real representation work, never a guessed value or native
layout/handle. Such unexecuted/unsupported contracts stay blockers. A successful
selected witness does not close all union/runtime/Copy/provenance/cleanup gates.
Use unchanged upstream fixtures and actual directives, plus LRR-only route/
alias/owner/cleanup/currentness controls outside originals. Review all8recurring
modes against an exact source fingerprint (MA002/MA003/MA006 are separate IDs).

Source review of unchanged union-drop-assign exposes a required distinction:
assigning a union field suppresses field Drop, but assigning a nested pointee/
struct field must retain its own Drop. MemoryView records whether its actual
parent descriptor is a Union. Direct union field stores and existing
needs_drop_type=false targets use initialization without reading inactive bytes;
other targets keep ordinary ReplaceOwned/cleanup and currently return an explicit
in-place-view cleanup gap until that continuation is integrated. Never silently
suppress nested Drop or count that original passed. Owning view reads snapshot
only the reached field slice, not an entire possibly much larger union region.
V448 build fails two exhaustive CellState cleanup matches; V449 lib build passes
but runs no tests. Those source fingerprints are not runtime acceptance.

V450 both builds pass; initial runtime source/control rows fail at actual owning
union result publication (13/214/34609dispatches in the three originals). The
new installer mistakenly called GuestStore::ticket, whose scalar-only admission
correctly rejects kind8. Use the same reserved TransferTicket allocation and
set_owner transaction as ordinary aggregate commit; do not relax ticket() or
wrap a foreign owner as a new ticket. These failures remain attached to V450.

Ordinary record pattern admission also excludes Union in pattern_declaration /
alias finalization and NominalConstructorTarget's schema binder. Extend their
actual record/nominal kind checks to Union, retaining all declaration/header/
namespace/variant/field proof checks. Matching, captures, guards and cleanup stay
in the existing transactional pattern engine, with fields provided by canonical
memory views. No union-specific pattern evaluator is introduced. The unchanged
align-struct record destructure is the minimal required witness; full pattern
coverage remains held until reached and tested.

V451 both builds pass; all three unchanged runtime cases reach their first
actual union field and fail the ordinary Named member admission at27/235/34700
steps. The prior attempted predicate edit selected the StableEncode match instead
of the actual prepare branch and changed no source. V452 applies the edit at
NominalMemberSelection::Named's real prepared-member branch with exact context;
all existing identifier/header/source/currentness checks remain. Retain V451
failure, do not guess field types from runtime name/shape. The origin control
adds an LRR-only adapter to the unchanged full item stream: a retained raw pointer
to u.a.0 must observe the later u.b write. This is an alias/identity control,
not an additional stock fixture or a substitute expected output.

V452 both builds/control terminal0;20edition/origin/retained-pointer worlds pass.
Owning2015 unchanged union-transmute and union-align pass; align-struct reaches
its union record destructure but fails the following `a.0 == 10` numeric demand.
Minimal invariant violation: binding_constraint_nominal::constructor_definition
still accepts only Struct in the *shared* pattern occurrence/source constraint
route, while pattern_names/nominal binder now correctly select Union. Extend that
actual declaration-kind admission to Struct|Union (including existing local
header path), retaining the same binder and field projection relations. No new
solver, numeric fallback, macro rule or pattern runtime. Query remains cold pure
preparation/memoized; installed matching/reads unchanged. Required evidence is
unchanged align-struct progress/completion plus union source/context controls;
unknown later library gaps stay held. Applicable recurring modes3/4/5 and audit
MA002/MA003; other modes reviewed separately at the final fingerprint.

V453 unchanged align-struct passes the union destructure/numeric assertion and
reaches Box::new(Align16(0)).as_ref(). The checked-in alloc facade lacks the real
AsRef impl present at pinned rustc library/alloc/src/boxed.rs:2407-2411. Required
T053/M018/M029 library-source prerequisite: add that ordinary trait impl/body to
the existing alloc facade, selected by common source member resolution and run
through existing Box dereference/reference operations. No method-name intrinsic,
lookup whitelist or new runtime opcode. Keep actual declaration/unit/edition and
source guard evidence; preparation is ordinary retained signature/body queries,
execution is one source call and reference projection. Do not claim allocator,
wide/provenance, dyn, Box/general AsRef or full fixture parity from this one body.

V454 actual upstream AsRef body runs; returning `self: &Box<T>` as `&T` fails
ordinary preflight_result_type_coercing. Existing explicit-borrow Box coercion
is not reached for an already formed reference read. Missing shared prerequisite
is builtin Box reference-value coercion, not a different AsRef body or method
rule. Extend GuestStore::unsizing_coercion/coerce_unsized_value (already shared by
typed storage, calls and returned values) with a validated builtin Box place
projection and ReferenceRecord::from_place. Preserve source ticket ownership,
actual owned_box identity, mutable-to-shared rules, flat location provenance and
validate/reserve/commit order. Use existing dereference_place/place_type_id and
lease constructors; no Deref method/name whitelist, forged lease, source lookup,
new runtime interpreter or new solver. Pure demand walks only the actual nested
Box chain; execution publishes one reference and transfers only its lease owner.
Required evidence: unchanged align-struct progress and existing ordinary nested
Box/reference coercion control, all-edition macro route controls. Full arbitrary
Deref/unsizing/dyn/cancellation and in-place view cleanup remain open; this is
T053/M018/M029 prerequisite, not a feature/card closure.

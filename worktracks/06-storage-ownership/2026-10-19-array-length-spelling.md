# T040/M018 — common integer spelling in retained array Type metadata

Baseline9541aedd162a852ebfb4c9a4d251971f131ec96b; solo LOCAL049, same-root
reservations. Unchanged upstream witness structs-enums/align-struct.rs reaches
AlignLarge { stuff:[0;0x10000] } after51559dispatches at V456, then its declared
field [u8;0x10000] remains unanswered and constructor specialization requests
unnecessary runtime Type context. No full original pass claimed.

Invariant: actual retained integer tokens have the same radix/suffix/magnitude
meaning in ordinary expression, pure constant and Type-metadata positions.
No display-text parser, guessed Type or template-context bypass may compensate.
Common ordinary route: actual TypeSchema/field range/header ownership ->
SchemaNames::actual_field_type -> source_types::resolve_actual_type ->
written_template -> shared record/member binder -> ConstructStart/Field/Finish.
Local fields use HeaderShape::capture over admitted actual AST. Signatures use
lower_type. Generated/handwritten syntax already joins these same retained AST
services with exact source/scope/edition/header guards.

Extend syntax::literal_decode with a small unsigned metadata projection from its
existing integer decoder (which uses pinned ra_ap_syntax IntNumber::value).
Replace source_types's decimal display-text parsing, signatures::lower_type's
decimal token parser, and declaration_field_types's separate token extraction /
unchecked u128-to-u64 cast. All consume the actual Literal AST and checked u64
magnitude. Preserve declared const-parameter identity and unanswered nonliteral
const expressions; do not add a new pure constant evaluator or source callback.
No static suffix/type/borrow/privacy rejection is added. This does not implement
all const expressions/generics or close those gates. Existing alias/member
routes remain held and are not promoted.

Cold cost O(actual literal spelling) once in ordinary memoized preparation;
installed operation execution does no source parsing, namespace census, whole
body copy, plan rebuild or inactive branch/callee preparation. No new limits,
resource accounting, sandbox, unsafe, parser/query/solver/backend. Existing pure
constant evaluator and ordinary runtime literal materialization remain distinct
by their specified roles; this only shares lexical decoding.

Acceptance: unchanged align-struct progression/completion through ordinary CLI;
source/header Type relations and generic/const signature controls; relevant
existing source/epoch/memo/cold checks and four editions/origins where exercised.
Unexecuted origin and full fixture entries stay gaps. Record all8recurring
failure modes separately from audit MA002/MA003/MA006 at the final fingerprint.
Only actual terminal/guard evidence may establish this held component; no task,
full interpreter/JIT, Linux, tooling or measured performance closure.

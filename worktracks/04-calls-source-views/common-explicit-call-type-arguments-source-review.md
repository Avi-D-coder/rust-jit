# Actual entered explicit Type argument consumer — LOCAL-024

Source-only investigation for WT04/T032/T033/M017/M018 under E-R5-005. No
production edits, Cargo, imports or runtime acceptance. Root owns Type/call
contracts/engine integration. The frozen goal/module components stay unchanged.
Applicable audit findings MA003/MA004/MA006/MA008 are separate from the eight
recurring failure modes reviewed below. This is the bounded preimplementation
architecture record and concrete handoff, not another Type parser/resolver.

The violated invariant is that an explicit Type argument belongs to its actual
caller body occurrence, scope, edition and hygiene. `fn main(){type u8=u16;
fn f<T>(){} f::<u8>();}` must not pick module primitive u8. Current common Q39
base-name selection can validly select f, but that fact grants no Type argument
authority. The existing Local normal-lowerer proof bypass plus
engine/run.rs::prepare_selected_argument_call using caller declaration
SignatureNames::unqualified(display_string) can lose the body-local alias.
Until the occurrence join below exists, this case needs the existing explicit
unsupported result; no primitive/string fallback or generic execution promotion.

Primary unchanged official witness remains the complete imported
macro-nested_stmt_macros.rs (344B, SHA
abef2c55f007cf929838a1cae6e3e329f8bf4747979db3e78b11580d5b86eb49):
grault wraps foo to emit real Bar/Baz/Xyzzy unit structs; the actual three
static_assert_exists::<Bar/Baz/Xyzzy>() calls target the ordinary module
function. An empty callee or unused T is not permission to omit source binding.
Official issue-75299.rs (152B, SHA
e22587ba68f83f67563d8ad741e769d295a10313f7cab7eccfbbb4de28bc4d02)
uses foo::<1> and local const N/array return. It remains a separate const purity,
argument association and array execution gap; a Type-only consumer must reject
its ConstArg rather than reinterpret it as a Type string. Originals unchanged.

## Exact existing route and missing association

EnteredCallGoal already proves owning BodyKey, call NodeId/site/range, actual
Call operand PathExpr NodeId, semantic BodyIndex identity, absolute syntax-tree
range and independently checked body-relative indexed range. Its
written_source_name is metadata only. The retained parent source has direct
ParsedFile::entered_node_syntax and BodyIndex::nodes_at_range. Use the actual
PathExpr AST selected by that proof, path.segment().generic_arg_list(), and
GenericArg::TypeArg(TypeArg).ty(). No reparsing target.explicit strings.
Pinned ra_ap_syntax 0.0.356 exposes the latter APIs in generated/nodes.rs.

For each actual TypeArg, admit supported ast::Type::PathType and retain the
exact PathType/path text_range. Its indexed node is NodeRole::Syntax, NOT the
call target Path expression node. BodyIndex construction assigns a node to
every nonopaque AST node, makes Item bodies opaque, and stores ranges relative
to body.range.start. Use checked subtraction of that base on the actual Type
range, index.nodes_at_range(relative), and the canonical Syntax NodeId that
PreparedBodyTypeOccurrences::prepare chooses. Confirm the exact direct AST
kind, enclosing TypeArg and GenericArgList belong to the admitted target path;
range containment alone is insufficient association. Do not assume generic
Type nodes are executable call.operands. No hardcoded NodeIds/ranges or whole
file ancestor scans. No actual numeric NodeId was observed by this worker;
retained/generated trees change coordinates and demand actual indexed capture.

PreparedBodyContext::body_type_occurrence(db,node) supplies the actual
EnteredBodyTypeOccurrence: path_range absolute, token context/edition, BodyKey,
shape, usage TypeSyntax and structural/provenance authority. The surviving
retained Original proof uses structural ledger first plus genuine raw source
token/root/AST owner/current lease. GeneratedBodyBoundary is distinct actual
generated authority; Mixed/Pending/NeedRetainedOccurrence remain held. Never
replace it with copied spans or normalize-as-raw coordinates. For Bar/Baz/Xyzzy
actual caller Type uses may survive original syntax, while emitted nominal
declarations independently need current generated/local row authority.

Capture one bounded immutable ordered list of (actual argument AST ordinal,
Type NodeId, absolute Type/path range, selected callee generic ordinal) under
the same Call goal/source/index identity. This is an association receipt, not a
new namespace or source context. Callee FunctionSignature.generics retains
DefinitionParameter.ordinal over ALL source generic parameters and GenericKind;
never invent ordinals by counting only Type args. For the simple type-only
case zip actual TypeArgs to genuine Type parameters using those retained
ordinals; retain explicit pending for lifetime elision, const/type ambiguity,
associated bindings, infer/composite/qualified Type syntax and general mixed
generic argument association until their shared mechanism is present. No
callee-name, empty-body or numeric whitelist.

## Shared Type facts and minimal API requirements

The same prepare_named_body_type(db,TypeNodeId) -> NamedTypePreparation runs
plan.type_prefix -> actual prepared lexical prefix -> current module completion
-> LocalTypePrefix::prepare_named. Lexical local/generic/import facts dominate
module/builtin fallback. Only COMPLETE actual Type namespace absence can
classify a primitive. This is the service the alias-u8 witness needs.

NamedTypePreparation already returns Ready(SourceType), Parameter(ParamKey),
Alias(AliasTarget), Name(SignatureName). A genuine local alias currently reaches
BodyTypeDemand::AliasContext before normalization and prepare_named returns an
explicit diagnostic. Preserve that refusal; contextual local alias preparation
is a separate common prerequisite, not module AliasNormalizer fallback.
Named lookup/module completion, actual owner generic/source publication and
generated/retained occurrence gaps must remain explicit.

Q42 EnteredTypeGoal currently retains ONLY Ready(SourceType). It explicitly
rejects Parameter as needing reached source substitution, Alias as contextual
normalization, and Name as incomplete resolution. Q43 PreparedTypePlan captures
the Q42 input closure once and retains its immutable goal/parent; the cast
consumer uses primitive_for(index,body,range). Thus a generic call cannot just
reuse primitive_for or extract a runtime tag. Minimal common extension is a
source Type observation capable of Ready(SourceType) OR Parameter(ParamKey)
(and explicit pending outcomes), with a use/association check that verifies the
actual Type NodeId/range/body/index but does not require primitive shape. Keep
primitive_for as a cast-specific consumer of Ready; it must refuse Parameter
until that consumer implements its own reached substitution. Q43 remains the
external guard wrapper; do not selfcapture Q42 or rebuild per phase.

A small call-specific prepared argument association may retain existing Q43
results/guards once, keyed by actual Call/site/target/signature and argument
ordinal proof. It must not duplicate TypeName/parser/index context or permit
caller-created SourceType. Expose a borrowed symbolic Type observation, not
printed text. Root chooses query integration; an external complete call plan
can combine Q44 selection and Q43 argument guards without selfcapture.

SourceType APIs already suffice for concrete builtin/nominal cases: source()
returns TyTerm, primitive() optionally projects a previously selected builtin,
find_descriptor(store) and materialize(store) use its certified TypeKey, and
require_descriptor validates actual pair. BodyTypeDeclaration::prepare proves
exact nominal declaration/source/context/header/edition and TypeSchema metadata
before SourceType::prepare_body_declaration. Nominal with source generics
currently returns a pending AliasTarget::Nominal; structural/generic Type args
remain gaps. No runtime descriptor can mint source authority.

For Parameter(ParamKey), the caller frame's accepted GenericEnv is the source
of both actual symbolic binding and optional runtime descriptor. GenericEnv
already exposes symbolic_binding(key) and runtime_binding(key), both binary
lookups; Frame only exposes parameter_hint (descriptor) and stores context
privately. Root needs one bounded checked Frame projection returning the
actual source binding plus runtime hint under matching caller definition and
accepted context/signature ownership. Do not reconstruct symbolic source from
TypeId. Do not simply transport foreign TyTerm::Param(callerKey) with a runtime
descriptor into callee GenericEnv: definition_numeric_context follows its own
environment's keys, which may lack that foreign key. Resolve the caller's
actual accepted source binding (bounded substitution/cycle checking if it can
remain symbolic), retain original occurrence/ParamKey as proof provenance,
and bind the callee's genuine ordinal to that actual source. No caller env
copy/reconstruction on installed phases.

## Reached descriptors and common call contracts

Prepare Type selection/association cold, retain immutable symbolic SourceType or
caller ParamKey. At reached call, require complete relevant guards BEFORE
argument selection or callee BodyStructure. For concrete SourceType use
find_descriptor or materialize only when existing normal call contracts need
the runtime descriptor; actual caller Param uses the accepted env projection.
Then produce existing ArgumentType {source,descriptor} at the genuine callee
ordinal, feed existing SignatureSubstitution::bind_type_parameter, call_contracts
source/runtime binding and GenericEnv::capture_current. The existing CallSeal,
argument eager order/suspension, frame body entry, return/cleanup remain owners.

SourceType nominal preparation fetches TypeSchema HEADER metadata; this is not
field layout. Runtime TypeStore::intern(TypeKey::Nominal) retains a deferred
nominal field recipe and CopyStatus::Deferred. SourceType::materialize can
therefore create the certified opaque descriptor for an unused T without
forcing field layout, allocating a value or lowering a body. Do not call
TypeLayout/field preparation merely because T is explicit or nominal. GenericEnv
currently requires matching source/runtime substitution table lengths, so the
minimal route may intern this opaque descriptor at the reached boundary; a
more general descriptor-optional environment is not needed for these unit
nominals. If a later Type family cannot provide an opaque descriptor lazily,
track that actual common prerequisite rather than eager layout or fake TypeId.

Replace the explicit-string SignatureNames/AliasNormalizer loop in
prepare_selected_argument_call ONLY for actual admitted source calls after the
prepared association exists; no fallback on missing proof. Retire the normal
lowerer's local_type ancestor/text checks once their safety role is fully
covered, not by a broad Local bypass. Preserve explicit unsupported until then.
A never-taken f::<...> call must not prepare callee executable code, layout or
argument evaluation; required source/header closure is distinct from execution.

## Evidence, cost and eight-mode source review

Required controls: full unchanged nested statement macro fixture in all four
editions and ordinary/user/wrapper/applicable stdmacro placement; actual
Call/Type NodeIds, ranges, contexts, selected emitted declaration identities and
normal lowering/CallSeal trace; alias-u8 same-source refusal/normalization;
caller generic forwarding with same source+descriptor; unrelated namespace
publication preserves relevant guards, relevant shadow/addition/epoch changes
revoke; cold never-taken source call avoids body/layout; noarg unused nominal T
retains binding without layout; exact-once args, suspension, failure/cleanup.
Const75299 stays separately unsupported. None executed by this worker.

Cold association is O(number explicit arguments * bounded tree depth +
log(index nodes) range lookup + shared memo Type/header/prefix demand); Type
lookup uses existing retained deterministic indexes and relevant namespace
dependencies. No new body scan/index copy or namespace census. Cache once;
installed operations consume fixed node identities and prepared closure guards.
Caller env projections O(log(number accepted bindings)), bounded symbolic
substitution. Reached descriptor interning uses existing store budget/interning;
no speed/scaling claim without measurements.

Separate source review against all8: (1) no assertion/operand evaluator, same
CallSeal; (2) actual attached syntax/ordinary Type+Call nodes, no scalar macro
lane; (3) real namespace/generic ordinal/source signature not name/numeric
context rules; (4) actual structural/hygiene evidence joins generated/retained
uses or explicit pending, no origin shortcut; (5) signature/Type/header metadata
only, no cold executable arm or callee/body/layout demand; (6) Q42/Q43 and
external call association retained once, no string parse/planning per phase;
(7) relevant local/module positive/negative guard dependencies, no world scans;
(8) direct actual node/range and Arc parents, no whole-file search or BodyIndex
copy. Existing string loop/lexical scan and their removal conditions are named
above; this report does not close a repair or generic compatibility gate.

## Inspected source fingerprints

These are moving-tree source-review identities only; later root/peer changes need
rechecking. No neighboring source/runtime pass is inferred.

- `crates/lrr/src/frontend/entered_call_goals.rs`: `ccfd2644459255b445343479bea9e9054cad9aa170f6049f01754483584875f9`
- `crates/lrr/src/frontend/body_type_occurrences.rs`: `6f65e48be0880ab8c02bdac1fa6864d9a899ae00a338558291fa2af922b0202d`
- `crates/lrr/src/frontend/entered_type_goals.rs`: `a28ce86d31b9754c0743bb71c21530aa684f865a84a8a0ac2c5c07147d0da2d5`
- `crates/lrr/src/frontend/source_local_plan.rs`: `f8fef4c1610ae330cda3a18bcb02281dca558fc967cfde64404b80af8749a491`
- `crates/lrr/src/frontend/body_type_declarations.rs`: `68c533d0168d9435805073d9a900639cb568f03fad10b94ce9a359f7735d96ea`
- `crates/lrr/src/frontend/source_types.rs`: `4da2a783184d70adc5a60d5bbf252c8baba3ba379206e5d703a4f9f4bdc2fd55`
- `crates/lrr/src/frontend/call_contracts.rs`: `4d9243a43d4e070ac16cb5b263f4b1d6d998b402f5cc50c0c7b304de37fb9e26`
- `crates/lrr/src/frontend/signature_subst.rs`: `9f921eaa790c657c39773662c5aef71c9aac5e4a8c853259a92f7d156cd363d8`
- `crates/lrr/src/engine/run.rs`: `475cb20a03a4893da192c03e2d5bac757d8c22c58691158a9100f87906a4e560`
- `crates/lrr/src/syntax/body_index.rs`: `c9988e93d581f89f703d7104902f314de2671a2518588b2e9817eb1ceed61bb4`
- `crates/lrr/src/syntax/node_lookup.rs`: `544d467f2f003fce9a46e1f3034983e1ba863e19fac7d9a54382899710029ece`
- `crates/lrr/src/syntax/signatures.rs`: `282b7c551edd7b1942efc24afcf45de83009c51e6c06c7f65dc2ef6a6853a462`
- `crates/lrr/src/vm/context.rs`: `5f284b2d833940da0b7d2f42d34188451419c6489d034538fc773cdbbbb272c7`
- `crates/lrr/src/vm/frame.rs`: `6e791921964933def4568f3a3a1b04f749ded8a030e81e83fb1037462fc39975`
- `crates/lrr/src/runtime/shape.rs`: `1199ab519b971c3fd7ea3713525986132b25ece7ff06b975c26376dbba615864`

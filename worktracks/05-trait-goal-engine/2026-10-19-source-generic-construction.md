# M019/M020 — actual declaration generics before typed fact freezing

Solo /root, LOCAL-049, baseline e5d461c4302a69b92783bb86acc8e8451efdd99c.
Minimal witness: the real shared TraitRef::new_from_args for the unchanged
traits/safety-ok.rs impl calls Context::debug_assert_args_compatible. The current
PreparedProvider::ordinary_layout reads DefinitionFacts, but those immutable facts
must themselves contain guarded TraitRefs. That construction cycle cannot be
resolved by fake facts, skipped guards or an empty SourceFacts provider.

Extend the actual common declaration route with a pure DeclarationGenerics query
keyed by the admitted definition. Observe actual DefinitionSource/Header/Context
and ParseFile. Capture through existing ParsedFile::declaration_generics, preserving
source ordinals, owning editions/hygiene and associated layout. TraitHeader must
retain the same query product rather than recapture its parameters. Exact generic
argument namespace/order metadata can be available before shared typed facts;
this is not trait applicability, inference or permission to solve a goal.

ImplSourceTypes retains those actual referenced declaration products. Build a
private immutable SourceGenericLayouts from the actual plans/catalog; scoped
configuration/provider construction borrows it. Ordinary argument guards consult
that real metadata during construction. Full typed publication must compare its
generic layouts/actual receipts to these source products. Missing metadata never
means an empty layout, success or ReadyProvider. Use the unchanged upstream guarded
TraitRef constructor once actual metadata is present. No arbitrary TraitRef literal,
fake SourceFacts callbacks, inference IDs, layout count guessing or debug-guard bypass.

Real root traits have implicit Self at upstream slot0, source written parameters
retain their ordinal, and immediate associated parents precede own parameters.
Function late/early region classification and const parameter Type metadata remain
explicit integration gaps; mere namespace/arity metadata does not manufacture
these typed facts. No body checking, whole-body inference, runtime parsing, source
IO/macro execution in callbacks, namespace census, BodyIndex copies or resource caps.

Cold generic capture runs once per demanded source declaration; retained query
products are shared via Arc. Constructor shape checks visit only relevant arguments
and generic kinds. No guest dispatch hot path is added. Actual immutable source
metadata alone must not make ReadyProvider/root evaluation available. Existing full
fact/inference/delegate/transitive dependency/runtime consumers remain held.

Witnesses: unchanged safety-ok, method-self-arg-trait, coherence-impl-in-fn and
method-two-trait-defer-resolution-2 source heads, plus existing borrowed/generic
trait fixtures; arbitrary macro/wrapper and all4editions through the same queries.
Check real TraitRef construction, Self/argument identity, shared generic query reuse,
actual namespace mismatch refusal, currentness/foreign world and no executable
preparation. Keep new runtime gain0 unless actual new stock execution is measured.
Read E-R5-005/E-R5-014 and current MA004/MA007/MA008 separately from the recurring
modes1-8; record a fresh exact-source review and async actual terminals. Full task,
interpreter, runtime/full-fixture/diagnostic/tooling/JIT/Linux gates remain open.

V326 actual shared constructor witness: unchanged coherence-impl-in-fn.rs's
ordinary `impl ::std::cmp::PartialEq for x` reaches the real declaration
`trait PartialEq<Rhs: ?Sized = Self>`. The current common projection_trait returns
an empty written argument list; its real source generic metadata requires Self
plus Rhs. The positive upstream constructor rejects it. This is a genuine missing
source-default prerequisite, not permission to weaken the guard or special-case
PartialEq/assertions/derive. V326 builds, but the Type control fails after two
successful source roots; retain its exact source and other controls separately.

Extend the existing WrittenTypeDeclaration/resolve_actual_type occurrence walk
with explicit earlier declared Type-argument bindings and actual trait Self.
Read missing defaults from retained TraitHeader.generic_ranges -> TypeParam AST,
in the actual trait's source/module/hygiene/edition; no display-text reparse,
synthetic signature or separate substitution walk. Complete arguments through
projection_trait's shared route for impl heads, projections, bounds/predicates,
associated equations and associated constant/nominal consumers, passing the
actual subject where known. Missing lifetime/const/default/projection facts stay
pending. No solver matching/inference algorithm is added. Both `Rhs=Self` and
arbitrary user trait Type defaults use this same source path; explicit arguments
remain distinct. No static rejection or eager method body preparation.

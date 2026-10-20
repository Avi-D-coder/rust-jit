# M020 associated declaration explicit-own recipes

Before source edits: clean pushed baseline `8c764548b1`, worker
`/root/wf_value_helpers`, parent `/root`. Own only
`frontend/declaration_source_predicates.rs`, `frontend/source_predicates.rs`,
new CLI `upstream_associated_predicates.rs` and this record. Parent owns Q80
production clause bundle/encoder and registrations. No Cargo/tests/commits.

Invariant/minimal witness: Q78 currently returns blanket AssociatedContext for
actual associated declarations even when authenticated parent, generic layout
and Self context exist. Unchanged `assoc-type-in-struct-constructor.rs` Tr/S/Tr::T
and `associated-types-basic.rs` Foo/Foo::T demonstrate ordinary associated
declarations; `generic-associated-types/streaming_iterator.rs` Item<'a> where
Self:'a demonstrates why child-own lifetime plus parent Self rebasing matters.
This slice is explicit-own source transport, not pinned own_clauses_of, inferred
outlives, full ParamEnv, item bounds, projections or runtime completion.

Common route (E-R5-005/E-R5-014): actual DefinitionSource/Header/Context and
DeclarationGenerics/DeclarationGenericTypes -> Q78 retained direct item AST ->
WrittenTypeDeclaration -> frontend/source_predicates::prepare_own -> existing
common bound collector/resolve_actual_type/projection_trait. Read actual
DefinitionHeaderPlan.SourceHeaderParent and retain actual parent source/generic
roots; verify parent/child layout, source/header receipts and inherited parameter
mapping. Resolve impl Self in its genuine parent declaration frame, rebase with
the existing parameter bridge; trait Self is the child's actual inherited slot.
No synthetic function/signature, text parser, name-hash ID or separate evaluator.

Own bound ranges/defaults stay owned by the child, and actual parameter spans
are retained for implicit Sized recipes. Parent requirements are not merged.
Associated `type X: Bounds` output bounds remain a separate item-bounds producer.
Remove the blanket associated hold only where actual joins validate; missing
parent/generic/producer evidence stays typed Pending. Existing source/runtime
member matching, solver root/factory and VM routes remain held and unchanged.

Demand/cost: one demanded immutable Q78 result owns Arc source/header/parent
roots and direct syntax locations. Parent Self/header preparation is cold and
memoized; encoding reads retained data with no source parse, full namespace
census, body lowering or guest work. No limits/resource accounting are added.
Currentness follows exact memo dependencies. No hot-path performance claim.

Evidence for component closure: independent actual Outline IDs/parent joins,
all four editions and original/user-emitted/wrapper origins, explicit-own versus
inherited clauses and Sized ownership, GAT Self/region ordinals or honest typed
gaps, retained spans, memo sharing, cold bodies and revocation. Frozen source
must receive separate eight-mode review: evaluator duplication, generated scalar
route, context member/numeric policies, generated rejection, eager executable
checking/pattern duplication, repeated planning, census/invalidation and immutable
body copies/searches. Runtime/root/ParamEnv/item-bound gates cannot close here.

Frozen implementation handoff (unexecuted source component):

- `frontend/declaration_source_predicates.rs` SHA256
  `057289b9ee5d57bebc25cbdcea8c9f8873bb0e43c222a40a6b0c7ca017750244`.
- `frontend/source_predicates.rs` SHA256
  `452158f9606f31ff1d3f3929464efec323e094d63e72a25492783baddcd16397`.
- `cli/preentry_loader_tests/upstream_associated_predicates.rs` SHA256
  `4823c0382f17abc525eaecae092996e012612b5ef867c4fd2579988a677ef045`.

Q78 now retains actual child DefinitionHeaderPlan, actual parent header and
DeclarationGenericTypes Arc roots, authentic parent definition, exact source
parameter rebase map, child-bound Self and own parameter source locations under
result encoding domain v2. New APIs: `parent_definition`, `self_type`,
`parameter_range(ParamKey)`, `source_header_root`, `parent_header_root`,
`parent_header` and `rebase_parent_parameter(ParamKey)`. ParentGenerics and
ParentLayout are explicit typed holds, with actual partial roots retained.
The original AssociatedContext hold remains for missing source/header/parent
evidence; it no longer rejects every associated declaration. No source_types
code was modified. Frontend source_predicates retains its shared own-selection
algorithm, with a clarified contract for associated callers.

Receipt checks: source child header agrees with its actual declaration source,
header and context; its SourceHeaderParent supplies the canonical actual parent
identity. Parent source/header/GenericTypes agree, parent tokens equal the
child's inherited context, inherited count and trait Self presence agree, and
every inherited parameter's ordinal/kind/name/bounds/default/const Type matches.
Trait Self maps to child own+inherited; impl Self resolves in its actual parent's
AST/source frame with the existing `rebase_parameters` map. Own generic ranges
are taken directly from the child's retained GenericParamList and validated
against own count. No inherited default or trait Self receives a child-own span.

Controls are external to unchanged imports. First selector
`unchanged_associated_explicit_own_recipes_keep_real_parent_self_and_sized_ownership`
intends 48 worlds / 108 associated declarations: the two primary Tr/Foo fixtures,
streaming_iterator (actual `Self:'a`, child lifetime and inherited I/Self), and
already imported check-pass issue-87750 (own GAT type parameter Sized/default
span). The latter is a source metadata witness, never a runtime fixture pass.
Parent impl I:Sized+Iterator/StreamingIterator recipes remain on the parent,
and the associated child's own list contains only its actual own requirement.
S and foo in the primary fixture retain their genuine own Sized+Tr recipes.

Second selector
`unchanged_associated_own_recipes_leave_parent_hrtb_unforced` intends 12 worlds
of unchanged higher-ranked/trait-bounds/assoc-type-projection-method.rs. It
selects actual Collection::my_iter via independent Outline parent/scope
attachments, asserts method Self slot/rebase and ready empty own source
recipes, and checks no parent Q78 request occurs during that demand. A separate
explicit Collection request retains its HigherRanked pending recipe. This does
not treat a parent recipe hold as successful full predicate closure.

Both selectors cover four editions and original/user-emitted/wrapper origins,
actual generated canonical IDs versus Outline auxiliary scopes, immutable memo
sharing/fingerprints, direct parameter spans, cold FunctionSignature/body/thunk/
pattern preparation and guards revoked by expansion epoch change. No tests or
Cargo were run by this worker. `git diff --check` returned zero. Parent owns
module registration, Q80/Q81 integration and all frozen validation.

Separate source review against all eight recurring failure modes, at the above
fingerprints:

1. Only source recipe preparation changed; no expression/assertion evaluator,
   executable code or solver algorithm was introduced.
2. Original/generated declarations share Q78, real source header/GenericTypes,
   WrittenTypeDeclaration and the same collector. No scalar/break macro route.
3. Parent/Self mapping is driven by actual declaration kind/source identity,
   never fixture name, method name, numeric or assertion context. Fixture
   expectations live only in the CLI source controls.
4. The blanket associated/generated hold is removed when authentic joins exist;
   missing joins stay typed. Whole emitted units retain actual canonical IDs and
   auxiliary member-scope evidence. Fragment producer joins remain separately held.
5. `prepare_own` filters bounds to the actual child range and own generic Sized
   ordinals. Parent requirements are not demanded; no body/signature lowering,
   unselected executable arms or pattern machinery is added. The HRTB control
   directly distinguishes child own demand from parent predicate demand.
6. Retained Q78 Arc roots and direct AST ranges do the work once per demanded
   memo. Parent source metadata queries reuse the existing framework; encoding
   reads retained data. No execution planning loop or reparsing is introduced.
7. Dependencies are actual source/header/generic/name query dependencies and
   existing PreparedInputGuards. No namespace census, dispatch map comparison,
   arbitrary derived input publication or unrelated invalidation policy added.
8. Existing Arc parser/query structures are shared. The rebase map and source
   ranges are cold immutable metadata; no BodyIndex copy or whole-file node
   search was introduced. No measured speed/scaling or resource-accounting claim.

The asynchronous reviewer separately inspected the parent/source receipts,
rebasing, generic/default ownership, absence of Q78 dependency cycles and the
own-versus-parent HRTB demand control; no blocking source finding remained.
These are source review statements, not executed evidence.

Semantic holds remain: Q78/Q80 are explicit-own source transport, not pinned
own_clauses_of, inferred outlives, implicit full clauses or elaborated ParamEnv.
An own written trait where-projection subject is retained as source syntax;
the pinned semantic filtering of identity associated projections into item
bounds is still a distinct producer. `type X: Bounds` output bounds are not
inserted here. Full higher-ranked/function region classification, fragment
parent producers, actual fact factory/admission/root, projection solving,
runtime dictionaries, interpreter acceptance and JIT remain held.

Read-only parent Q80/Q81 review after handoff (no production edits/tests):

Observed SHA256s: `source_declaration_clauses.rs`
`86da0bca278cf1fe83bfe426b2216c5d531a99f2409b078812eb23965d89cb6a`;
`generic_clauses.rs`
`2573862a2923d56b312a5cfcdb3ce14df46f2bd8eaa0434a05480e8b1fef8b43`;
upstream `source_predicates.rs`
`79bdc0acd53f7965245327161b23fdde779e14e59d319276d67966118929ce09`;
new CLI `upstream_source_clauses.rs`
`16b5c8ac796c31d45fd25be19edf9643b85d2c3677b968f0eb15b14694dd9a1a`.
Parent made only the host `parent()` helper pub(super) for reuse in the owned
CLI; its new SHA256 is
`b7c6f432b998cb4863ea93dfd035ed4ea79cfd214328997856a878629f7f2499`.
The earlier prehandoff CLI fingerprint above remains attached to that source.

No blocking source issue found in the inspected production/control delta.
Q80 captures own Q78 recipes, actual binding requirements and source parent
generic/header identity; it does not request parent recipes. Q81 separately
walks the actual associated parent hierarchy and carries a parent pending result
without fabricating a partial successful closure. Both encode through the same
require_recipe/encode_recipe transport used by impl source recipes. The old cfg
projection helper observes production Q80 instead of rebuilding source bindings,
guards or encoder loops. Source clauses remain ExplicitOwnClauses, a different
type from FullClauses, and do not grant readiness.

The actual streaming iterator control observes the real StreamEnumerate impl
and Item<'a> source pair: parent I has source ordinal 0/upstream index 0; child
inherited I has source ordinal 1/upstream index 0; child own 'a has source
ordinal 0/upstream index 1. Pinned instantiate_identity preserves parent source
identities and parent-before-own spans. Pinned instantiate with the genuine
child identity arguments replaces parent I with the child's inherited I, while
the child's nominal Self argument and own early region remain their actual
child source parameters. Own instantiation excludes the two parent clauses.
The cfg control uses the Q78 rebase receipt to observe these real identities,
not caller-supplied clauses or a fake provider. Context encoding/instantiation
checks zero source query delta.

The HRTB control inspects Q80 child readiness/empty own source list and no parent
Q78/Q80/Q81 query before explicitly asking Q81; that separate hierarchy request
returns Parent pending, whose retained actual Collection bundle explains its
HigherRanked recipe. No signature/body/thunk/pattern preparation is introduced.
Currentness evidence scope: new CLI first revokes guards by epoch change, then
publishes empty inputs and confirms they remain revoked. This does not isolate
source-withdrawal invalidation independently of epoch revocation; a fresh guard
would be required to claim that separate control. All commands/tests remain
unexecuted by this reviewer.

Eight-mode delta review: no evaluator/assertion route (1), scalar/break macro
route (2), fixture/context method/numeric policy (3), generated refusal (4),
pattern/body/unselected-arm checking (5), repeated execution planning/source
queries during encoding (6), hot namespace census (7), or BodyIndex copying/
whole-file location search (8) appears in the inspected source. Cold Q80/Q81
preparation retains Arc roots and encoded vectors with separate data/context
lifetimes. This is structural review, not speed/scaling or runtime acceptance.

Next genuine item-bound producer integration point (unimplemented): use the
actual trait-associated member from Q77 plus its source Header/GenericTypes and
Q78 parent/Self/rebase receipt to construct the real associated projection
identity arguments through the existing SourceBindings/source_terms encoder.
Extend the common retained AST bound collector with an explicit item-bound
subject/mode for TypeAlias.type_bound_list; do not reparse tokens or synthesize
a function. Keep this source bundle separate from Q78 own declaration where
requirements, Q81 parent closure and full compiler item/self/nonself queries.

The immediate shared representation gap is projection Sized defaults: current
SourcePredicateRecipe::ImplicitSizedDefault and encode_recipe's
ResolvedSizedDefault handling require a ParamKey/source generic range. An
associated output default is instead Sized(<Self as Trait>::Item<...>) with its
actual associated declaration span. A shared span-bearing default recipe for
an arbitrary authenticated Type subject is required; do not use an own generic
parameter default or a fabricated literal bound to stand in for it.

Port/retain pinned collect/item_bounds.rs::explicit_item_bounds_with_filter
(lines 422–490) query applicability and associated_type_bounds (lines 25–112),
including real projection identity args, implicit sizedness/default-trait leaves,
parent-trait where-bound selection, GAT remapping/nested projection traversal
and PredicateFilter. Trait-associated declarations are supported inputs;
ordinary free type aliases have the pinned known empty outcome, while unrelated
impl-associated items are not silently empty. No generic-count-zero shortcut
exists here. Opaque/RPITIT source identities remain typed dependencies.

The pinned parent filter only imports the appropriate own-trait associated
projection with identity arguments; it does not import arbitrary projection
subjects from every parent clause. Preserve source HigherRanked/const/associated
equality gaps until their actual producers exist. `item_bounds` then elaborates,
`item_self_bounds` elaborates and filters Self, and `item_non_self_bounds` takes
the actual set difference (pinned lines 493–525); do not label the explicit
source list as those full answers. Runtime witness already imported:
associated-types/associated-types-bound.rs GetToI32::R:ToI32 and foo's call
through the actual projected output. The primary Tr::T fixture also witnesses
the real projection Sized default even without written output bounds. These
are proposed next controls, not executed or accepted features.

Parent source advanced during the read-only pass. Re-read the changed cfg
parameter observer (now an explicit lifetime-parameterized function) and
hierarchy StableEncode (now records the retained parent hierarchy payload
directly, rather than the whole QueryValue wrapper); both preserve the reviewed
behavior. Updated observed source_declaration_clauses SHA256:
`b12d9c982b1a63e1a34e83974f16fe1bb5de2c677cf9c81aa7793dffce964ef9`.
Updated parent-owned formatting/public host CLI SHA256:
`ff49a04478f531c586e00198e1df88793297693fbd3f9da36dbb2be7881bb06b`.
The initial review hashes above remain attached to their earlier source; bind
the final asynchronous review to the parent's eventual global freeze rather
than treating this moving-source inspection as a frozen-build receipt.

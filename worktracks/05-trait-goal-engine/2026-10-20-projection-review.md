# M020 projection source and TypeIR review

Independent review/test owner `/root/wf_validation_review`; source baseline
`289995a5accac76fcb386b46d863e1118cbb6e6d`. The final V498 source review is
bound to fingerprint
`16c5d5dbee02245bcb283daf850ffcdcf2fbe941291ac05eccda91de2b642b41`
(8,674 guarded inputs at the same HEAD). The implementing lead transferred sole
Cargo ownership after the explicit final source freeze; terminal evidence is
pending. This source review includes both new pure queries, production binding/
encoder/generic readers and all selected control helpers at that fingerprint.

The V499 review recheck is bound to
`9f2913e0a577d7a92a3cbdc6c523255901fb3191e65be2425657256a94da0240`
(same HEAD and 8,674 inputs). Its two source deltas remove the unnecessary
`.as_str()` call from actual AST name text and complete manifest
`support_sources` to match the registered exact source tuple. The source-route,
authority and all-eight-mode conclusions carry to V499. V498's failed build and
audit remain attached to the original fingerprint; V499 terminals are pending.

This bounded component concerns the actual source identity of associated Type
declarations and transport of unnormalized projection Types/trait bounds into
the pinned shared TypeIR. It does not select an implementation, normalize a
projection, construct a complete source-fact provider or enter an upstream root.
The minimal current witness is unchanged `repr128.rs`: the `discr` declaration
has `<T as DiscriminantKind>::Discriminant: PartialEq<U>`. The unchanged
`union-generic-rpass.rs` independently contains `MaybeItem<T: Iterator>` with
`ManuallyDrop<T::Item>`. Existing name-only associated-item keys cannot stand
in for their authentic declaration identity.

## Ownership and evidence plan

Workers own the new source projection plan and TypeIR encoder changes. The
lead owns new query registration and actual declared nominal Type payload in
`source_definition_headers.rs`. This reviewer writes only the new separate
`cli/preentry_loader_tests/upstream_projection_types.rs` test module and this
review. The lead registers that module. Shared test setup is the existing
`configured_impl_catalog_world`; real captured source/namespace/declaration
receipts are required. No fake provider, source spelling whitelist, complete
factory readiness assertion or fabricated associated declaration is allowed.

Primary source controls preserve imported originals and their exact item bytes
inside arbitrary emit/wrapper macros. They cover owning editions 2015, 2018,
2021 and 2024; actual source IDs, source range/parent joins, shared memo identity,
guard withdrawal and executable-code coldness must be asserted. Standard-macro,
procedural, generated-associated-item, runtime normalization and complete root
matrices remain unexecuted unless separately evidenced. Runtime regressions
and source metadata controls must retain separate outcomes.

## Existing common routes and migration holds

Before this candidate, `frontend::source_types` retains shorthand/explicit
projection `TyTerm::Projection` records with `associated_name_key` identities.
`source_terms::Encoder::ty` explicitly returns `ProjectionMetadata` for them;
`source_bindings::require_type` does not collect their trait/item/argument
closure. Existing `frontend::associated_types` implements reached concrete
normalization through the legacy member index and remains migration debt.
The source projection component must resolve an actual associated declaration
through the common retained source/name/generic services and preserve its
separate parent-trait and item arguments, without promoting that legacy
normalizer or inventing a second trait solver.

## Source-query review pass

`SyntaxProvider::TraitAssociatedTypes` (tag77) ->
`PreparedTraitAssociatedTypes::prepare` fetches actual `DeclarationGenerics`,
`TraitDeclaration` and `PreparedScopeImplHeaders`. It derives the authentic
member scope from the original declaration or actual `GeneratedSyntax`
Outline attachment. `declaration_for_member_scope` must join that scope to
the requested canonical trait ID; auxiliary generated Outline IDs do not
become the published declaration ID. Actual `ScopeMembers`, `ScopeBlockers`
and `ScopeBucket` agree before a retained TypeAlias declaration and its owning
token edition are accepted. Same-parse members validate their actual AST trait
ancestor; cross-parse admitted fragments require the generated parent join.
The retained map converts `(parent trait, item-name key)` to actual declaration
IDs. `lookup` rejects any pending plan before exposing an item and returns
typed `ItemUnavailable` for an absent key. Neither result is solver failure.

This source slice was read at worker handoff hashes
`39d192ff6aafdc1114ab90dd890e8726af771db215e731611657cb0ee19a58f0`
(`trait_associated_types.rs`) and
`5766d840998143d12c0dcb244dc03e3732ae617d70fb429ac9766593c025c8ee`
(`source_predicates.rs`). Whole-candidate review fingerprints are recorded above.

The frontend predicate delta removes unconditional projected-Type refusals
from ordinary source subject/argument/outlives collection. It preserves actual
unnormalized terms for the shared source binding closure; unresolved associated
equalities, qualified/callable/HRTB syntax and incomplete source remain typed
pending. Existing consumers and remaining legacy projection normalization still
require their own migration; source recipes do not make complete ParamEnv facts.

## Eight-mode source review

1. No assertion/expression evaluator is introduced. Projection declaration
   identity is a demand through ordinary source/declaration/name queries.
2. Original and arbitrary emit/wrapper syntax join the same declaration query.
   The positive generated-member parent route uses the actual published unit;
   no literal-only or scalar generated execution is added.
3. Trait/item IDs come from exact scopes and actual source buckets. A legacy
   item-name digest remains a lookup key, never a DefId. No macro identity,
   primitive discriminant rule, method whitelist or numeric hint selects it.
4. Generated traits/members retain owning parse/profile/token edition and
   canonical source joins. Missing joins remain typed pending. Arbitrary
   associated-item macro fragments and the complete standard-macro matrix
   remain unexecuted requirements, not inferred neighboring passes.
5. Selected declaration headers, aliases and generic layouts are read without
   lowering bodies or checking unselected executable arms. No new pattern
   machinery, full static checking, borrow checking or normalization is added.
6. The existing pure query retains source/query Arcs, declaration map and
   relevant bucket dependencies. Cold preparation walks each demanded trait's
   Type members and local AST ancestors; repeated lookup reads a retained map.
   Runtime/installed execution must consume these results, not re-enter them.
7. Per-trait namespace inputs and ordinary bucket queries are source preparation
   dependencies. No full-world dispatch census or unrelated runtime invalidation
   is introduced. Index/source catalog preparation remains cold; there is no
   measured performance claim.
8. Existing retained parser/Outline/query handles and direct syntax locations
   are reused. Local ancestor walks do not search the whole file per member.
   No BodyIndex clone, second parser, unsafe code or resource cap is added.

## TypeIR integration review pass

`source_bindings::SourceRequirements` separates declaration IDs from unresolved
`(trait source ID, legacy item-name key)` demands. `require_type` traverses
projected Self and trait/associated argument Types in the same structural walk
as nominal/tuple/reference/array Types. `SourceBindings::prepare` resolves the
actual trait member through Q77, then requires both real declaration receipts
and `DeclarationGenericTypes` before publishing `SourceProjectionBinding`.
Missing lookup results remain `ProjectionMetadata`; they cannot become empty
bindings or semantic NoSolution. `projection_parent_matches` checks actual
trait Self, captured inherited/own generic layout, parent header tokens and
TypeAlias syntax, names/kinds/ordinals/bounds/const Types. Existing source
collection/readiness limits remain explicit.

The follow-on `SyntaxProvider::DeclarationSourcePredicates` (tag78) ->
`PreparedDeclarationSourcePredicates::prepare` retains the actual
`DeclarationGenericTypes` and source inventory. It selects the declaration's
direct AST item and calls `WrittenTypeDeclaration::declaration` followed by
the shared `source_predicates::prepare_own`. Own parameter/range filtering uses
the existing bound collector; the actual captured trait Self slot supplies
supertrait subjects. No synthetic function/impl header is manufactured. Missing
Self is explicit, and declarations requiring inherited associated rebasing
return `AssociatedContext`. This query supplies own source recipes, not inherited
predicate closure, elaboration or a complete ParamEnv.

This second source pass reads handoff hashes
`8ef5db1af8d36322480c4fa0af7c2f070e1eb85640c8806c9125d3b901ae6b5e`
(`declaration_source_predicates.rs`) and
`cccb9d31bd082553cb0c0076ce4d2c46e49ec904730a17f2b2e23bb837c5acf7`
(`source_predicates.rs`, superseding its earlier source-slice hash). The
all-eight-mode conclusions apply to this same retained source route; associated
inherited rebasing remains an explicitly unexercised producer requirement.

`SourceGenericLayouts::from_bindings` rejects different catalogs, inconsistent
IDs/query roots or pending bindings; it retains real associated argument kinds
and trait-parent IDs. `PreparedProvider::{parent,alias_layout}` read these
construction-only maps before the existing fully prepared semantic tables.
`ready` admission is not relaxed. They add no source query or runtime metadata
reconstruction and preserve explicit missing-data failure for unadmitted facts.

`source_terms::Encoder::ty` reads the resolved binding, translates Self followed
by actual parent-trait and item arguments, checks the real alias argument-kind
layout, and invokes pinned `AliasTy::new_from_args` before constructing an
unnormalized `IsRigid::No` Type. The pinned constructor still runs its debug
argument compatibility check. No independent normalization or primitive result
Type rule is added. The nonescaping actual source context and immutable generic
tables remain the owners; encoded arena handles cannot escape their lifetime.

The changed encoder/binding call sites were enumerated: actual impl heads,
impl source clauses, ADT field Types and generic const-parameter Types all pass
the same resolved `SourceTypeBindings`; they do not keep a projection-specific
encoder. The existing frontend `PreparedImplPredicates` still calls the shared
collector's original `prepare`; Q78 uses its `prepare_own` view. The remaining
direct collector call sites are host controls. This delta adds no runtime
dispatch consumer or alternate normalization path.

The parent nominal-header delta separately constructs genuine nominal declared
Types from the actual ADT identity and existing compiler identity arguments.
`DefinitionHeaderInputs::nominal_type` rejects nonnominal roles, foreign worlds
and mismatched real generic metadata. It does not demand fields, infer function
Types, fill complete DefinitionFacts or remove unrelated semantic holds.

These changes extend the same source binding/encoder route across the eight
modes above; existing alternate runtime projection/member routes remain named
migration debt. Whole-source fingerprints are recorded above; build/test
terminals are separate evidence. No acceptance follows from this source-only pass.

## New controls and review boundary

The separate CLI module uses independent actual Outline attachments and actual
generated publication IDs. A generated Outline DefKey supplies the genuine
member-scope attachment, while the map key remains its published declaration
ID; these identities are deliberately not conflated. Imported originals remain
unchanged. Their exact item bytes are wrapped in arbitrary emit/wrapper macros.

Three source component selectors cover 12 union projection worlds (24 field
Types), 12 repr128 projection-bound worlds (36 own clauses including 12 projected
subjects), and 24 direct associated-declaration worlds from unchanged
conditional-dispatch and associated-types-basic. The last includes actual
generated local `Foo::T`, distinct impl TypeAlias IDs and a typed missing-member
lookup. Each selector checks a current immutable memo and guard withdrawal,
and excludes executable FunctionSignature/BodyStructure/Thunk/PatternPlan
demand. The TypeIR helper must independently inspect actual alias IDs/arguments
and obtain its recipes from Q78; direct assertion outputs cannot substitute for
that route review. These numbers describe required assertions; actual terminal
outcomes are recorded separately below and never inferred from this test source.

The arriving helper source was separately inspected. The field helper consumes
actual retained `MaybeItem` field terms and materializes their existing ordinary
field binder. The predicate helper consumes Q78's real source recipes, builds
their required declaration closure and calls the shared `encode_predicate`.
It retains a joined guard over the actual source/configuration/catalog/generic
and associated-declaration query closures. Neither helper manufactures an impl,
predicate/projection recipe, layout table or complete provider. The private
`check_actual_projection_alias` checks unnormalized Projection kind, exact
associated ID/world/trait parent, actual Self parameter, real constructor-kind
compatibility, a rejected missing Self argument, and pinned
`trait_ref_and_own_args`. The predicate helper also checks actual positive
PartialEq clause identity and its distinct U parameter. Encoding records no
source queries and `ready` stays refused before and after it. CLI tests explicitly
fetch Q74/Q78 before and after helpers and require the same cacheable Arc and
fingerprint, separately from guard currentness.

Only the missing-member and wrong-argument negative assertions above are planned
for these selectors. Foreign/ambiguous/producer-incomplete associated metadata,
nonempty GAT item arguments, higher binders, inherited associated predicate
rebasing and ordinary runtime normalization remain unexecuted requirements.
The positive TypeIR witnesses project actual core Iterator/DiscriminantKind
members across root origin worlds. Q77 additionally exercises generated local
`Foo::T` identity. That source-query result does not establish TypeIR binder
construction for a generated associated declaration, which remains unexecuted.
The existing positive field matrix changes its expectations in this candidate;
V497's 72 encoded/12 pending receipt stays bound to its original source.

## Per-kind scope

| Kind/route | Selected evidence or hold |
| --- | --- |
| Actual associated Type declaration ID | Q77 original/facade and generated local Foo::T controls; missing-member is source pending. |
| Unnormalized projection TypeIR | Actual Iterator::Item and DiscriminantKind::Discriminant aliases with one Self Type argument; root original/emit/wrapper worlds. |
| Nonempty associated item arguments, region/const projection arguments, GATs | Representation code is reviewed; these combinations are unexecuted. |
| Own free-function source bounds | Q78 actual discr source recipes; positive projected PartialEq<U> plus two source Sized clauses. |
| Associated inherited own bounds / full inherited-elaborated closure | Q78 explicitly holds inherited associated context; complete predicate publication is not provided. |
| Associated equalities, higher-ranked, callable or qualified bounds | Existing explicit recipe pending paths remain; no new equality solver or binder inference. |
| Nominal declared Type payload | Struct via existing header matrix; enum/union via new cold nominal matrix. Complete factory fields remain held. |
| Function/constant/static/type-alias declared Type payload | Typed NonNominal result; actual semantic producers remain held. |
| Actual upstream root / selected dictionary / runtime normalization | Unimplemented integration; ordinary runtime replay is reported separately. |

The table identifies the chosen assertions and remaining per-kind requirements;
it does not establish a passing outcome before the terminal sections below.

## V498 terminal scope

The Rust 1.99.0 offline, locked library no-run build returned 101 for E0658 at
`frontend/trait_associated_types.rs:147`: `actual_name.text().as_str()` invokes
unstable `str_as_str` because the text value is already a string slice. No
production repair was made by the reviewer. Full inference, language-vocabulary,
transmute and nominal source audits returned zero. The bootstrap audit returned
one with `solver inventory pin/closure differs`.

No CLI/digester build, selected control or runtime replay was attempted for
V498. All processes were terminal before closing guards. Source and HEAD matched
the before receipt at the exact V498 fingerprint, with no changed guarded
inputs. The closing binary receipt contains only the unchanged **old V497 CLI**
SHA256 `ac355e83fc027d344fd87e7a098e26f3cd1e613dd99735691cf3e9872963fb7d`;
it is not a successful V498 build. No current library test executable was
claimed. Exact logs/guards are under `artifacts/validation-current-v498`.
Source/Cargo ownership returned to the lead after the closing guard. All new
control assertions remain unexecuted at this fingerprint.

## V499 terminal scope

At fingerprint `9f2913e0a577d7a92a3cbdc6c523255901fb3191e65be2425657256a94da0240`,
offline locked Rust 1.99.0 library/CLI/digester builds returned zero. Full
inference, bootstrap, language-vocabulary, transmute and nominal audits returned
zero. V498's compile and inventory failures are not counted as V499 acceptance.

The projected-field selector passed (29.030 seconds): 12 edition/origin worlds,
24 actual MaybeItem field Types, exact Iterator::Item ID/parent/Self argument,
retained memo reuse, no encoding source queries, cold executable bodies and
revoked-guard rejection.

The projected-bound selector returned 101 (4.908 seconds). Actual repr128
2015 original source passed its three own clauses/one projected subject, then
generated origin1 failed during Q78 preparation with `LRR-E-CALL-CONTEXT`,
`generated declaration has no nominal/alias Type recipe`. Read-only inspection
located the existing generated Type-name owner admission match in
`frontend/generated_result_namespace.rs::lookup_type_declaration_using`, which
admits impl/nominal/alias/trait owners but lacks the actual free-function header
route. Its later `lookup_declaration_inner` retains real generated row/header,
token-range and namespace proof checks; those checks are not a bypass target.

The direct associated-declaration selector returned 101 (11.844 seconds).
Conditional-dispatch Deref::Target passed all three 2015 root origins, and
original Foo::T passed, before generated Foo::T returned `MemberScope` pending.
Its authentic generated parent join succeeded, but Q77 then found one or both
ScopeMembers/ScopeBlockers input rows absent. No state dump establishes which
row was missing independently. The actual generated unit retains its
Outline/canonical declaration map; ordinary generated member namespace
publication/currentness must supply the common missing source join. The test
does not replace it with spelling or auxiliary Outline identity.

These two unchanged source matrices expose mode-4 coverage gaps in common
generated declaration/name and member-source joins. They hold generated
projection feature/architecture completion. The next repair must extend those
shared immutable joins while preserving actual source/token/namespace proof;
a header-only spelling exception, synthetic function, second projection parser
or original-only narrowed acceptance would not satisfy the common pipeline.

The nominal cold selector passed (59.050 seconds): 24 worlds and 48 enum/union
headers, with source memo/currentness/revocation and no field/schema/body/
signature/thunk demand. Both failing selectors still reported exactly one
selected body and unchanged guarded inputs. No source repair occurred during
this validation.

The unchanged two-case runtime batch returned one runtime-contract pass
(`generic-derived-type`, owning 2015, stdout `10\n10\n`, all main/capture/cleanup
checks true) and one `repr128` interpreter failure (`LRR-E-OP-TYPE`, owning
2015). The report records no native execution and unchanged original/manifest/
binary inputs. This remains ordinary interpreter evidence, not upstream solver
root or complete factory execution.

The existing header matrix passed (93.722 seconds): 36 worlds, 276 root headers,
24 requested facade headers and zero requested facade gaps. The updated field
matrix passed (86.057 seconds): 36 worlds, 96 encoded fields and zero pending
declarations. This establishes the positive selected MaybeItem projection
branch at V499; it does not rewrite V497's historical pending receipt or establish
complete source fact/root/runtime integration. All six control receipts report
one selected body and unchanged inputs; the combined selection returned one
because two selectors failed.

All V499 processes were terminal before closing guards. Before, after-build and
after-runtime source/HEAD checks match the exact V499 fingerprint above, with
no changed guarded inputs. Test SHA256
`fcc75872d26d81c2e2f952bc6296cdcb7a2b36e8eb75d44ff39cd9bc0131e6bb`
and CLI SHA256
`88c26dc2a9ebc3b4355ac7ef1990d90db835f9f3ed9fc23d026b16cda0d4bfaf`
match the build receipt after all controls/runtime cases. Exact commands/logs
and guards are in `artifacts/validation-current-v499`; runtime report is
`/private/tmp/lrr-v499-projection-runtime/report.json`. Source/Cargo ownership
returned after the closing guard. The two common generated-source prerequisites
and remaining per-kind matrices stay held; V499 does not establish complete
projection/root/full-fixture/architecture or interpreter/JIT acceptance.

## Post-V499 common-join repair review

This source pass concerns the subsequent candidate, not the immutable V499
fingerprint/evidence. Final whole-source fingerprint and all new terminals are
pending. The parent extends generated Type-name owner admission with actual
function `DeclarationGenerics`: exact definition/function kind, parse/profile,
header, scope and published DeclarationContext must agree. The same existing
generated row/header/token-origin/namespace proof core then resolves the name.
No FunctionSignature or body query, synthetic function or context-specific
name lookup is introduced. The draft `ItemKind::Fn` typo was reported before
freeze; the actual manifest variant is `ItemKind::Function`.

`GeneratedSyntaxInput` now retains `scope_contributions` prepared once from
its real admitted Outline. Every candidate's auxiliary Outline ID must join to
the actual published declaration ID; blockers remain actual source blockers.
The exact map participates in stable encoding. This is a source-owned retained
contribution, not permission to assert namespace absence or producer completion.

`Database::publish_discovery` authenticates each private generated unit against
the actual discovery seal and dependency record before computing namespace rows
internally. The arbitrary derived-input allowlist is unchanged. Contributions
merge into affected scopes; generated member-scope collisions require the same
actual parent. The same `granular_scope_rows` projection serves original and
generated publication. Unchanged NameCandidates/ScopeBlockers fingerprints are
preserved so unrelated bucket guards need not invalidate merely because another
candidate appears. Trait-wide Q77 plans intentionally depend on that selected
trait's complete member list.

The coordinator retains the original base ScopeMembers Arc for affected scopes.
Epoch advance removes contributed granular names/blockers, restores the base
scope and its granular rows, and clears generated member-owner tracking. Full
source reset clears those base/owner maps. New generated facts therefore must
not survive epoch withdrawal through a stale positive name bucket. This was
inspected in source; authentic publication, multi-unit composition, missing-name
invalidation, unrelated bucket stability and exact base restoration require
the separately owned terminal regression.

Against all eight modes, these extend the existing generated source identity
and pure namespace publication core. They introduce no expression evaluator,
literal-only output execution, method/numeric policy, pattern engine, static
checker or installed-runtime planning. Missing associated-fragment producer
completion remains held rather than clearing blockers from a seal label.
Preparation retains Arcs/direct locations and performs no BodyIndex copy or
whole-file node search per lookup. Additional cold work clones/merges affected
scope candidate/blocker maps and normalizes them; repeated incremental root
publication can revisit that root map. No dispatch/global-namespace scan or
measured speed claim follows. The relevant cache invalidation behavior must be
measured by the actual granular-input regression before acceptance.

The new local projection-field helper and two new CLI selectors were inspected
at worker hashes `ebc43a4dda7fed2d6b0a23786d639d481e534d3e33fb9529b62f433fde4d454b`
(`source_adt_fields.rs`) and
`aef32dcf25ea39fcf476b9f6134db2bad517bbd874e3592ac0bfdc72a071c7eb`
(`upstream_generated_projection_fields.rs`). Unchanged official
`assoc-type-in-struct-constructor.rs` and
`packed-with-inference-vars-issue-61402.rs` supply actual local trait/member/
field declarations. Their original/emit/wrapper worlds require genuine generated
publication IDs, distinct trait/impl aliases, parent-first binders of one/two
Type arguments, actual Self parameter/nominal Type and usize trait argument,
and the actual Box wrapper where present. Both positive witnesses use existing
field source terms and Q77 bindings; no ProjectionKey is manufactured. Helpers
call pinned alias construction/argument splitting, reject empty Self arguments,
leave readiness closed and record no encoding source queries. Memo, cold and
withdrawal checks are required across all four editions. These 24 worlds are
planned component assertions until terminal; they specifically target the
previously unexecuted generated associated-declaration TypeIR route.

The final V500 recheck is bound to
`00628ea6f43e24ba37ee37f27a3218381037c26f3ce0087e72774875608180d9`
(8,676 inputs; HEAD `289995a5accac76fcb386b46d863e1118cbb6e6d`). Exact
changed paths from V499 are the three common production publication/name files,
the field helper, two new CLI modules and their existing test registration/setup
module. All production paths and selected helpers were reviewed at this frozen
fingerprint. The final namespace control additionally prefetches actual Left,
Original and NeverEmitted buckets before publication and retains their guards.
After authentic two-unit publication, Left must invalidate, while unrelated
Original/NeverEmitted retain their current guards and memo Arcs. Its positive
Left guard must revoke on epoch change; the reused query must return original
empty candidates/blockers, and nonroot generated scope/blocker rows disappear.
The test keeps actual original blocker facts throughout, making no unsupported
producer-completion inference. Source reset removes the remaining universe.
Four owning editions are required. Actual terminals remain pending; independent
multi-batch/procedural and unresolved associated-fragment producers remain held.

## V500 terminal scope

At the V500 fingerprint recorded above, offline locked Rust 1.99.0 library,
CLI and source-digester builds returned zero. Full inference, language-vocabulary,
transmute, nominal and normal bootstrap audits returned zero. An additional
`bootstrap_trait_solver.py --verify-upstream` audit returned zero and verified
the local pinned Git objects: 221 files, 3,574,451 bytes, nine crates and zero
retained-original Rust changes; registered stable adaptations remain explicit.
The normal bootstrap log is preserved separately from this Git-object audit.

The first three selectors passed: projected Iterator fields (29.213 seconds;
12 worlds/24 fields), actual discr projected PartialEq<U> own bounds (28.644
seconds; 12 worlds/36 own clauses/12 projected subjects), and associated
declaration identity (56.171 seconds; 24 worlds). The latter two crossed the
V499 generated free-function and generated Foo::T failures through the common
source joins. These are source/TypeIR component contracts with memo, guard and
cold assertions, not solver root or runtime execution.

The unchanged two-case runtime replay remains one generic-derived-type
runtime-contract pass (owning 2015, stdout `10\n10\n`, all actual main/capture/
cleanup checks true) and one repr128 interpreter failure (owning 2015,
`LRR-E-OP-TYPE`). It records no native execution and unchanged original/manifest/
binary inputs. No checks are inferred for the failed row from its absent checks
object. This is ordinary interpreter evidence, not execution of new source facts
through a ready upstream solver provider.

The nominal cold selector passed (56.670 seconds; 24 worlds/48 headers), the
header matrix passed (96.465 seconds; 36 worlds/276 root/24 requested facets,
zero requested gaps), and the updated field matrix passed (85.978 seconds;
36 worlds/96 fields/zero pending declarations).

The local shorthand Tr::T selector returned 101 (4.868 seconds). Original 2015
source passed its actual alias/parent/Self/binder assertions; generated origin1
returned `Source(SourceDefinition)` in field source binding. Its independent
Q77 member query had already passed. Read-only source inspection suggests the
remaining `source_types::projection_bound` manually constructs a member scope
with canonical trait ID, while generated members use the actual auxiliary
Outline owner. It skips that missing scope and the existing shorthand path
retains a zero unknown-trait key, which cannot receive a real source binding.
The exact intermediate source term was not dumped, so this is a source-based
diagnosis rather than observed internal state. The required repair is the same
authentic common member-scope join, preserving hygiene/currentness; no fake
projection or convenient generated-only lookup is allowed.

The explicit generic Trait<usize>::Assoc field selector passed (28.121 seconds;
12 worlds/12 fields), including real generated trait/associated IDs, actual
two-argument parent-first binder, nominal Self S, usize, Box source identity,
memo/revocation/cold and pinned constructor checks. This establishes that
selected generated associated-declaration TypeIR branch at V500; shorthand
selection and nonempty item/GAT/binder matrices are not inferred from it.

The publication selector passed (10.035 seconds; four editions/two actual macro
records each). It verifies exact canonical scope/granular merges and preservation
of original blockers, relevant candidate-arrival guard invalidation, unrelated
bucket memo/guard stability, positive guard revocation and reused-query absence
after epoch restoration, original namespace preservation and source reset.
It grants no unresolved producer completion or general procedural authority.

All nine control receipts report one selected body and unchanged inputs; the
selection returned one for its single failing selector. All V500 processes were
terminal before `after-runtime`: source/HEAD match the before/build fingerprints,
no guarded inputs changed, and test SHA256
`caa89821bb0c50e897bad0c6d912588608bdec1ea76bbc15f111862012b64be6`
and CLI SHA256
`b9f3ab656f9866abf0f5da9b743c05161a1513b836757d81cecd271ba0ecdf19`
match after-build. Source/Cargo ownership returned after these closing guards.
Exact logs/commands are under `artifacts/validation-current-v500`; runtime report
is `/private/tmp/lrr-v500-projection-runtime/report.json`. The remaining common
shorthand generated join keeps feature/architecture completion held. Eight
passing component controls, provenance and one unchanged runtime pass are not
complete factory/root/full-fixture/interpreter/JIT acceptance or a new stock gain.

## V501 shared trait member-scope review

Final source review fingerprint
`0ffc8527d5916f45f60f951c80f40e2cda82cca8060c26b46e3e9b09f9cb461e`
(8,676 inputs; same HEAD `289995a5accac76fcb386b46d863e1118cbb6e6d`).
Four guarded paths changed from V500: `trait_declarations.rs`, Q77's
`trait_associated_types.rs`, the ordinary `source_types.rs` bound selector and
the local generated-field CLI epoch assertions. Earlier evidence remains bound
to its own source.

The existing immutable `TraitDeclaration` query now retains one typed
`member_scope` receipt and its actual generated source Arc. Generated traits
must match the exact admitted unit/declaration, parse/profile, HeaderRecipe,
DeclarationContext range/scope, Type/Trait kind and unit parent map. Original
traits retain their actual source owner. Generated parse metadata without its
unit join returns `GeneratedParentJoin`; it cannot fall back to a guessed
original owner. The receipt and source enter stable domain v4. A missing member
join is independent from establishing that source syntax is a trait, and grants
no namespace completeness or semantic negative result.

Q77 removes its duplicate auxiliary-scope derivation and reads this common
receipt while retaining inventory and parent-map verification. Ordinary
`projection_bound` now reads the same receipt instead of constructing a member
scope from the canonical generated trait ID. It preserves its actual bucket,
blocker and trait-argument checks; unknown source remains pending. The legacy
signature-text fallback and runtime normalizer elsewhere remain migration debt,
not new or accepted routes. No parser, normalization solver, generated special
case, primitive-name result or readiness relaxation is added.

All eight modes were rechecked for these actual call sites: no evaluator,
literal-only output, contextual method/numeric behavior, new original-only
refusal, pattern/static/body checking, installed planning, global runtime
namespace census or BodyIndex copy appears. The one cold trait query retains
its direct declaration/map join; consumers use a borrowed cached ScopeKey and
the same granular bucket service. The additional cost is cold receipt creation
and retention per demanded trait; no speed claim is made.

The field controls additionally capture the actual TraitDeclaration guard.
After epoch advance, field/member guards must reject old facts, and generated
trait receipts must also reject. Source inspection verified the real transitive
dependency chain: TraitDeclaration -> ParseFile(GeneratedSyntax) -> DiscoverySeal
-> ExpansionEpoch. The provider's existing stale-epoch check supplies that
dependency; no invented manual epoch proof is needed. Subsequent complete
source withdrawal still rejects field/member guards. All new assertions and
the justified changed-source rerun remain awaiting actual V501 terminals.

## Read-only next prerequisite findings during V499 validation

The current positive TypeIR controls use actual original core associated
declarations under varied root origins. Direct Q77 additionally covers generated
local Foo::T publication IDs. Do not infer generated associated binder TypeIR
construction from those neighboring cases. Already imported unchanged
associated-types/assoc-type-in-struct-constructor.rs (SHA256
8b7183304d6ee634fb3484db7aa3a00ece816496df723a904754198475de58b3) supplies
local Tr::T in S<I:Tr>(I::T), suitable for that next common field-path control.
The imported packed/packed-with-inference-vars-issue-61402.rs (SHA256
26624f3e897b252e8e44629c699a1f42df0fa886ba077ec9fc6bdd9d2a86db3c)
adds a real generic trait argument. Both match pinned checkout originals; neither
new control was executed here. GAT item arguments still need the existing
source_types::shorthand_projection generic-argument refusal repaired first.

Associated macro fragments need an actual publication bridge. Existing
PreparedGeneratedAssociatedDeclarations validates aliases/parent/request/epoch
but has only test callers. completed_source_syntax generated_associated_members
currently admits Function/Const; discovery publishes GeneratedSyntax only for
item outputs, and RetainedSyntax.generated_owner requires a body-bearing
function/constant. Q77 cannot safely accept a TraitItems grammar as a parent
receipt. Reuse the existing authenticated association producer to publish the
actual alias DefinitionSource/Header/DeclarationContext, Type member candidates
and producer completion. Its real parent must also feed the common associated
generic collector; an AST-fragment-only parent would lose inherited generics
and implicit Self. Do not relax Q77 or add projection-only ordinal guesses.

Exact pinned, unimported/unexecuted source witnesses are
tests/ui/parser/issues/issue-65041-empty-vis-matcher-in-trait.rs (check-pass,
574bytes, SHA2564ec164649aa92b5e41654253a3e3831f37ecfc7b9762483e29eef7871836c205)
and tests/ui/macros/issue-22463.rs (185bytes, SHA256
907d8f946b8405f96486a8c7d8aa3ef816f91c46f9111cf4bbcb89d97f205d9c).
These identify source/architecture prerequisites, not new runtime or full-fixture
acceptance. Worker investigations made no source edits/copies during freeze.

## V501 terminal scope and closing guards

Validation completed against source fingerprint
`0ffc8527d5916f45f60f951c80f40e2cda82cca8060c26b46e3e9b09f9cb461e`
(8,676 inputs), HEAD `289995a5accac76fcb386b46d863e1118cbb6e6d`.
`artifacts/validation-current-v501` records the before, after-build and
after-runtime guards, Cargo JSON, logs and exact-selector receipts. Offline
locked Rust 1.99.0 library no-run, CLI and source-digester builds all terminated
zero. Full inference, bootstrap, language vocabulary, transmute and nominal
source audits terminated zero. The separate bootstrap `--verify-upstream`
terminal was zero: 221 files, 3,574,451 bytes, zero retained-original Rust changes
and `upstream_git_objects_verified: true` at the exact pinned upstream revision.

All nine selectors in `2026-10-20-projection-controls.json` terminated zero with
one actual test body and unchanged inputs. In selection order their measured
times were 29.228, 28.368, 55.459, 55.750, 94.062, 88.511, 27.818, 28.087 and
9.977 seconds. This covers 12 Iterator-field worlds/24 fields; 12 discriminant
predicate worlds/36 own clauses/12 projected subjects; 24 direct associated
identity worlds; 24 nominal cold worlds/48 enum or union headers; 36 header
worlds/276 root headers/24 source facets/zero requested gaps; and 36 common
nominal-field worlds/96 fields/zero pending fields. The two local unchanged
associated-field fixtures each passed 12 worlds across all four editions and
original, emit and wrapper origins. Their real alias identity, parent, Self,
arguments, retained memo, currentness, epoch invalidation, source withdrawal and
cold-path assertions ran. The previously failing generated shorthand `Tr::T`
case is positive here, rather than inferred from the explicit generic trait
case. Namespace publication additionally passed all four editions with two
actual emitted records each, original-scope preservation, relevant/irrelevant
bucket guards and epoch restoration.

The unchanged owning-2015 runtime batch used `LRR_EMBEDDED_FACADE=1`, the fresh
V501 CLI and source digester, the same two selected upstream cases and report
directory `/private/tmp/lrr-v501-projection-runtime`. It terminated one:
`generic-derived-type.rs` was a runtime-contract pass with stdout `10\n10\n`
and all actual entry/capture/cleanup checks true; `repr128.rs` remained an
interpreter failure, `LRR-E-OP-TYPE`, with no successful-check record. The report
records no native execution, an unchanged imported-original/manifest/binary
guard, incomplete runtime acceptance, failed selected batch and guest JIT held.
The component controls do not upgrade that runtime result, establish a complete
semantic factory/root, cover associated-fragment producers or GATs, or close the
remaining architecture and full-fixture matrices.

All processes were terminal before the closing guard. Source and HEAD matched
the before guard with zero changed inputs. Test binary SHA256 was
`07faa4a911534e199ac8a3ecc9d578d7c4615527f7d68b2907dd43be714f020f`;
CLI SHA256 was
`730c905983e5df71ec8157fdf9592ef6fe5d52eb27472ac12bd67290781eff91`.
Both matched their after-build identities. Source/HEAD/binary ownership was
returned to the lead after that terminal; this review update changes only the
worktrack record outside the validation source manifest. Historical V498–V500
failures remain attached to their own fingerprints.

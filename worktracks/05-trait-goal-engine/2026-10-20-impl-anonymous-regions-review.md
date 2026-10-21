# Anonymous impl-region independent source review (unexecuted)

M019/M020 / LOCAL-049. Read-only review of the parent's common source producer;
no production edits, Cargo, compiler runs or Git operations. The parent and
validator own the actual source controls. The unchanged acceptance target remains
repr128; imported impl-header lifetime fixtures are separate source controls,
not runtime/full-fixture passes.

## Inspected compiler behavior

Pinned revision ae0fe511b15ab760d7e963b13389d1bc2ab42b18, actual compiler files:

- rustc_resolve/src/late.rs SHA256
  `0c8560d97898126597085247c6255a1ba166c35859d9ba9e7a2a03613e5127e9`.
- rustc_ast_lowering/src/item.rs SHA256
  `4726eb0e6cb374dcd56a6e6fab6ed786d0181c8bce149d989771841c88375050`.
- rustc_hir_analysis/src/collect/generics_of.rs SHA256
  `901423a7bc0e566b5464b756862a35d11d7530fd5fbe2ca1ae1dd70d2bdc9452`.

`resolve_implementation` (late.rs3568+) puts trait arguments and self Type under
one AnonymousCreateParameter rib, visits trait arguments before self and then
visits generics. `resolve_anonymous_lifetime` (1968+) calls create_fresh_lifetime
for that rib; create_fresh_lifetime (2216+) creates a distinct node and appends it
to the binder's extra list, with no unification by spelling. lower_generics
(item.rs1803+) appends those extra lifetimes after written parameters. generics_of
(271+) groups all early lifetimes before Type/Const while preserving their
relative order and prefixes genuine parent parameters. This is declaration
representation, not borrow checking or region solving.

## Positive source conclusions

`syntax::signatures::capture_impl_generics` uses the shared AST and appends one
anonymous Lifetime parameter per genuine omitted/underscore RefType or explicit
LifetimeArg underscore site. Sorting trait/self roots by actual source start
matches the pinned trait-before-self order. Written parameters keep original
source ordinals; appended anonymous parameters get later ordinals. `&&T` and two
separate underscore arguments have distinct ranges; RefType lifetime syntax is
not also LifetimeArg, so its underscore is not counted twice.

`header_syntax::ImplHeaderSyntax::prepare`, declaration_generics and
associated_generics_with_layout all use that same producer. Associated items
inherit the parent's same site-bearing parameters after their own written
parameters. Existing `$crate` spelling pairs written parameters only, filtering
anonymous entries; it does not shift actual written bound syntax onto a new
anonymous parameter. DefinitionParameter's range joins declaration/source
identity retained by the normal query; a bare range is not a global identity.

`upstream::parameters::DeclarationParameters::new` separately maps source order
to upstream order: parent prefix first, then own; early lifetimes first in each
group, preserving Type/Const order. Trait Self remains the true first upstream
slot while the original source identity convention is retained. The inverse map
round-trips source ordinals without renaming them to upstream indices.
`source_generics::early_region` uses name None for actual anonymous entries and
keeps their genuine SourceParameter identity; it does not substitute Static or
invent a named region. materialize uses these same upstream vectors and actual
parent_count. Reviewed declaration_generic_types compares inherited
anonymous_region provenance as well as kind/name when rebasing parent parameters.

Additional unchanged-at-review file fingerprints:

- traits/header_syntax.rs:
  `250214b808314e2d259d4a84188fbe1ee4116b7d9cf4d1d84628b93ca4decde8`.
- traits/upstream/source_generics.rs:
  `788363fcddfac61477ec034e0ed17f8a3343cd4dc662f8caab504807219f03bd`.
- frontend/declaration_generic_types.rs:
  `750ef6d60707d3c390d271aed7bc3060b3bdeb6f4f98df4ff65da8019054719d`.

Nested FnPtr and ForType subtrees are excluded from impl parameter collection,
and Expr subtrees exclude const-expression-local declaration work. No executable
bodies are traversed. Explicit named/static reference lifetimes add no anonymous
parameter. Function late/early region classification remains separately pending.

## Finding corrected during this review

Initial signatures SHA256 `49429b39eba6ad81a17049a262936ac404a18e404a4f9abf60e89d5437317b18`
descended through `dyn Fn(&u32)`. That reference belongs to Fn's implicit
PolyTrait binder, not the impl. Minimal source witness:

```rust
trait Tr {}
struct H<T: ?Sized>(T);
impl Tr for H<dyn Fn(&u32)> {}
```

Pinned visit_poly_trait_ref (late.rs1098+) establishes a PolyTrait rib;
visit_path_segment Parenthesized (1400+) resolves its function signature with
that binder. The initial preorder would append a false impl region even though
the source encoder separately held Dyn metadata. This was reported immediately.

The parent's observed correction skips DynTraitType/ImplTraitType subtrees and
adds impl_region_binders_pending for unknown macro/object/opaque binder domains.
DeclarationGenerics withholds its complete layout; DeclarationParameters returns
ImplRegionBinders. Thus a held nested domain is not silently certified as zero
impl parameters. Corrected signatures fingerprint at this read:
`31b7891a5469174addedbb618a8d4a7a5ff5877d3d1bd7764b7d1e7e3daf7373`;
DeclarationGenerics `bab8e38f9160ac42d4dae2114b314f5df47c3f3a6ebea2a0d5d9d0f1d87b2a82`;
parameters `fcd0a4e08b0e3562c66545f7985d0c46cab24bd8eedc293c1f20d3d9e0f5b55f`.
Those are in-flight source observations, not frozen compilation evidence.

## Remaining explicit completeness requirement

Pinned resolve_elided_lifetimes_in_path (late.rs2286+,2384+) consults actual source
declaration lifetime arity and creates a fresh parameter for every omitted path
argument. For example `struct W<'a>(&'a ()); impl Tr for W {}` needs one impl
parameter although its self Type contains neither RefType nor LifetimeArg. The
syntax-only collector and current nested-binder predicate cannot establish that
arity. This was separately reported to the parent. The branch must retain a
typed metadata hold until the ordinary demanded declaration-arity producer has
joined; a later encoder failure is not evidence that a zero generic count was
complete. Do not reject the source statically or add a special name/fixture rule.
The parent's implementation record already names unrepresented paths as held;
actual source-arity enforcement and source controls remain to be demonstrated.

Parent source_types/preflight and declaration parameter-site joins were still
being edited and were not part of a frozen final review. In particular the shared
runtime Ref walker intentionally retains its historical Static fallback; exact
source preflight/site proof must prevent that fallback from becoming solver
metadata whenever an anonymous declaration region is required. This review does
not claim that final join passed.

## Eight failure modes and acceptance boundary

1-2: shared declaration/header AST/query services only, no assertion/generated
expression route. 3: no member/numeric inference rule or trait solver added.
4: same actual source scopes and range-bearing parameters for handwritten and
admitted generated syntax; no origin exclusion. 5: no patterns or cold-body
checking. 6: retained AST/header queries, no parser or installed-operation phase
planning; associated queries can repeat a linear parent-header walk, which is
preparation cost rather than a claimed measured optimization. 7: no world census
or runtime namespace guard added. 8: no BodyIndex copies or whole-file node
searches. Costs are linear in demanded header Type syntax/parameter count and
the source/upstream permutation; no release performance claim follows.

All source controls, generated/user/wrapper contexts, actual encoded distinct
region identities, dependent invalidation and Cargo remain unexecuted in this
review. Parent validation must freeze the final source after its remaining joins;
the source fingerprint changes supersede only affected conclusions. No root,
runtime, full-fixture, compiler-diagnostic or interpreter/JIT gate is closed.

## Final delta pass: path arity, const arguments and predicate sites

At source_types SHA24218de4f3facd0d9eaec1f8780e63fc608f2cfa144c97cd6602d8d5328fd756,
DeclarationGenericTypes SHAefd4194fe32fec668413aa6009b939717aba33a067fbc5559d0ef378c32be347,
and declaration_source_predicates SHA98ebf0ee045f29de94fda295a5b3d379a432ad36e3661622ad4d597039486fe4:

- impl_path_regions_pending resolves actual source declaration targets, fetches
  raw DeclarationGenerics for lifetime arity and returns Parameters through the
  typed GenericTypes producer on missing arguments. It creates no hidden params.
  SourceBindings explicitly refuses pending GenericTypes, and HeaderPlan/typed
  layouts consume that binding path. Raw DeclarationGenerics remains syntactic
  source metadata; its list alone is not a complete typed binder certificate.
- Arity walking now applies only to a direct Impl using that impl's own source
  receipt. Associated GenericTypes fetch actual parent GenericTypes first,
  preserving parent pending through the normal inventory/generated owner join;
  only its actual raw parent metadata is used for const-type rebasing. This avoids
  using child source-token authority on parent AST. Exact parent header/context
  comparison and parent anonymous-site equality remain mandatory.
- written_generic_argument carries declared ConstTerm::Param and substitution/
  rebase rather than evaluating/defaulting a const. An intermediate blanket
  complete_trait_arguments Const hold was reported and corrected: full arity
  matches Const/Const, and partial completion carries only provided actual consts;
  absent const arguments remain held. No const evaluator was added.
- declaration_source_predicates adds actual own anonymous ranges at their appended
  ordinals, rejects collisions, and checks total own source sites. Associated
  rebasing compares parent anonymous_region alongside original bounds/defaults/
  const-type recipes. It uses site-aware source preflight/region lookup for parent
  and own self Types, preserving real ParamKey rather than runtime Static.

### Additional const namespace blocker reported before final stamp

The same source still promotes a matching const parameter in any TypeArg before
ordinary Type namespace lookup. Pinned late.rs::visit_generic_arg1332 explicitly
resolves TypeNS first for a single argless identifier, and falls back to ValueNS
only on actual TypeNS absence. GenericParamKind::Const is inserted into the value
rib, so a global source Type can share its spelling. Minimal valid source shape:

```rust
trait Tr {}
struct Name;
struct W<T>(T);
impl<const Name: usize> Tr for (W<Name>, [u8; Name]) {}
```

`W<Name>` must retain the real source Type Name while the array length uses the
const parameter. The premature TypeArg const promotion loses that distinction.
Additionally impl_path_regions_pending skips a matching const name in every
PathType context, even a true trait/self Type path. For `trait N<'a> {}` plus
`impl<const N: usize> N for X<{N}> {}`, this skips the trait's actual omitted
lifetime arity. Both need the same ordinary TypeNS-first/absence proof and an
actual generic-argument occurrence context; explicit AST ConstArg PathExpr may
directly transport a declared const. No custom inference or static rejection is
appropriate. These were sent to the parent; final no-findings stamp is held until
the corrected source is read. The examples are source-review witnesses, not
executed compiler or LRR controls.

### Dependency/cycle observations

Direct impl GenericTypes -> retained PreparedScopeImplHeaders/environment ->
normal Type namespace lookup -> target raw DeclarationGenerics does not recurse
to target GenericTypes, HeaderPlan, predicates or solver. PreparedScopeImplHeaders
itself reads ScopeDeclarationInventory and indexes source headers. Associated
GenericTypes -> actual immediate parent GenericTypes strictly moves to the actual
Impl/Trait parent; those have no associated parent. The AST/metadata joins must
stay exact so malformed parent discovery cannot become self recursion. Raw
DeclarationGenerics remains syntax-only, including its ParseFile and source
inputs. DefinitionHeaderPlan -> SourceBindings -> typed GenericTypes therefore
has no newly observed direct semantic-query cycle in these branches. Do not move
target arity resolution into raw DeclarationGenerics or make it require its own
typed HeaderPlan to solve a new cycle.

The real db.fetch/read edges record preparation dependencies; NeedInput/errors
propagate rather than becoming false absence or zero arity. SourceFactBundle's
retained guards/current catalog check remain the outer currentness owner. This
pass is structural evidence only: source-dependency changes, namespace absence
invalidation, changed target arity, generated parent/currentness and cycle
diagnostics need actual controls. No runtime callback may discover this metadata.

All-eight review for these deltas: shared source/header/query entry points only
(1,2); normal Type/Const namespace distinction must be repaired as above rather
than a context-only override (3); source sites/hygiene preserved in same route
(4); demanded header Types only, no patterns/cold bodies (5); immutable source
query preparation, no runtime planning (6); ordinary retained namespace lookup,
no new installed-entry census (7); retained Arc/query handles and direct source
ranges, no BodyIndex clone or repeated file search (8). Cost is one linear
demanded header walk plus normal memoized target arity/parent queries and source
parameter maps. No Cargo or runtime/performance acceptance follows.

### Namespace and actual const substitution correction read

The parent added WrittenTypeDeclaration::ambiguous_const_parameter, which uses
ordinary resolve_name_at and selects the declared value parameter only for exact
Unresolved(NameLookup::Absent). NeedInput/errors/ambiguous or unknown Type lookup
never count as absence. The same helper is used by written_generic_argument and
the path-arity walker, whose skip is now limited to an actual AST TypeArg parent.
const_parameter itself requires a single argless nonabsolute path; the normal
single-segment absolute Type path uses existing module_path_type. This matches
the pinned TypeNS-before-ValueNS branch and resolves the namespace findings.

On re-reading the new partial-default branch, the ArrayType PathExpr length was
still manually reconstructing a ParamKey and could lose a provided actual const
substitution in a type default such as `[u8; N]`. Parent replaced it with the same
const_parameter helper and removed that manual key reconstruction. Observed
source_types SHA256:
`ad34a5a177817b1867293a78896e4596b97bef81b01fe7ae294f269a51b93691`.
This is source substitution/rebase transport, not a new const/default evaluator.

### Argument-shape check initialization dependency

Parent additionally added Encoder::require_arguments for genuine nominal/trait
argument count and kind, preventing omitted fn-input path lifetimes from becoming
empty-argument IR. At source_terms SHA256
`99303ce08885e48d201a1eff306d350dd3c71fde6da267a3ce7c236875683fe4`, its call to
cx.generics_of introduces a concrete initialization-order problem:

`with_provider -> SourceGenericLayouts::materialize -> encode_declared(const Type)
-> Encoder::require_arguments(nominal) -> PreparedProvider::generics_of ->
generic_facts.with`, but generic_facts.enter(prepared) occurs only after materialize
returns. A nominal in the already-existing const-parameter Type branch therefore
hits an unentered scoped table. Primitive usize controls do not exercise it.
This was reported as an actual dependency-order blocker. The check should read
the actual retained construction argument_kinds (or return typed ParameterLayout
when unavailable), without querying the materialized table under construction.
No recursive generic query or fake ready scope is appropriate. Final stamp for
this new argument-shape branch awaits the corrected source read.

### Final bounded source stamp before V529

Re-read the corrected source; no actionable finding remains within these bounded
deltas. PreparedProvider::source_argument_kinds first checks the actual source
world and reads settings.construction_generics.argument_kinds, available before
IR materialization. Encoder::require_arguments uses that immutable kind vector,
holds ParameterLayout on absence and compares exact count/kinds; it does not read
the unentered materialized generic table. The prior initialization-order finding
is resolved. The alias control supplies genuine SourceGenericLayouts through
with_source_context; no empty/unverified layout is substituted.

Final source SHA-256 values:

| File under crates/lrr/src | SHA-256 |
| --- | --- |
| frontend/source_types.rs | ad34a5a177817b1867293a78896e4596b97bef81b01fe7ae294f269a51b93691 |
| frontend/declaration_generic_types.rs | efd4194fe32fec668413aa6009b939717aba33a067fbc5559d0ef378c32be347 |
| frontend/declaration_source_predicates.rs | 98ebf0ee045f29de94fda295a5b3d379a432ad36e3661622ad4d597039486fe4 |
| syntax/signatures.rs | 31b7891a5469174addedbb618a8d4a7a5ff5877d3d1bd7764b7d1e7e3daf7373 |
| traits/header_syntax.rs | 250214b808314e2d259d4a84188fbe1ee4116b7d9cf4d1d84628b93ca4decde8 |
| traits/upstream/parameters.rs | fcd0a4e08b0e3562c66545f7985d0c46cab24bd8eedc293c1f20d3d9e0f5b55f |
| traits/upstream/source_generics.rs | 788363fcddfac61477ec034e0ed17f8a3343cd4dc662f8caab504807219f03bd |
| traits/upstream/source_terms.rs | 7eaadc04f8fcc62b64e34f78b87622e0b6de191d1e48d67e7036622439f51c2a |
| traits/upstream/prepared.rs | 8adfe5c1db64be1ad911a47a42d3545bf6d6223b69d71d537fca8be55a3a18f9 |

The eight-mode review and dependency/cycle observations above were rechecked
against these sources. New kind/arity checks are immutable representation checks
at preparation, not static language refusals, a solver, runtime source access
policy, namespace census or resource cap. Query target/parent dependencies still
flow through normal memo fetch: Database::returned records DependencyStamp::Query
even for cached outcomes; actual source-input reads/NeedInput propagation remain
unchanged. Actual invalidation/generated-origin/cycle controls are required and
unexecuted; structural inspection does not prove those runtime behaviors.

Only source/hash and documentation terminals ran in this independent pass; no
production edits, Cargo, tests, compiler invocations or Git operations. Parent
was notified that V529 may freeze this exact candidate. No source-root activation,
runtime/full-fixture, diagnostic/tooling or milestone acceptance is inferred.

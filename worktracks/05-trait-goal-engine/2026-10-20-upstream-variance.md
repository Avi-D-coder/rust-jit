# M020 pinned variance inference over actual source fields

Before-code bounded record. Baseline `3d25d2b`; owner edits only new
`traits/upstream/variance/` modules, `tools/upstream_variance.py`, the exact
pinned variance support originals and their bootstrap/inventory registration.
The query-domain owner retains `prepared.rs` and `source_variance_queries.rs`;
parent owns module registration and field source accessor. No Cargo or commit.

Invariant: generic nominal variance is compiler semantics, not a default
covariant/invariant vector derived by a second algorithm. Reuse pinned
`rustc_hir_analysis/src/variance/{terms,constraints,solve}.rs` from
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, including lattice/fixed point,
mutability/function polarity, alias handling, lang overrides, const invariance
and unused-function parameter policy. `mod.rs` is retained for query-domain
provenance. The unchanged primary witness is imported
`tests/ui/generics/generic-derived-type.rs` (`Pair<T>` has two actual `T`
fields). Existing original/emit/wrapper and edition source controls are the
appropriate observations, not a new miniature variance language suite.

The common route is actual DefinitionHeader identity and source inventory ->
memo-owned AdtSourceFields plans -> retained field Types and generic metadata
-> authenticated nominal field-dependency closure -> source-only configuration
context and actual `SourceGenericLayouts` -> immutable typed input handles ->
pinned terms/constraint/solve implementation -> owned variance result. Query
domain preparation retains each source/query root and its currentness closure;
no callback parses source or performs queries, IO, body preparation or guest
execution. Ordinary and generated declarations use the same field service and
actual canonical IDs. Source ownership and generic binder namespaces remain
those of the existing source receipts.

Only compiler-context operations (HIR enumeration, actual IDs/kinds/generics,
field/type/signature lookup, local/foreign classification, arena and output
storage) are adapted. Mechanical generation/audit must compare complete retained
algorithm bodies to the pinned originals. No independent solver, cycle
scheduler, normalization, covariance shortcut, resource cap or builtin-name
variance selection is allowed. Missing foreign variance, lang identity,
signature or unsupported typed input is explicit pending before algorithm
entry, never an empty field list or a fabricated provider. Root facts and
runtime/JIT admission remain held.

Cold preparation follows actual field-type dependencies; mutual local nominal
dependencies enter the pinned simultaneous inference domain. Unrelated
functions/bodies are not prepared. Projecting the upstream crate item domain
onto the demanded closed field-dependency set changes only source enumeration,
not constraints or the fixed point. Foreign items require real completed
variance inputs rather than local guesses. The adapter uses retained immutable
handles; hot algorithm work is the pinned constraint walk/fixed point. No
performance claim is made. Eight-mode source review and exact source hashes
will be recorded at freeze; asynchronous validation is separately owned.

## Implemented source component and bounded review

`variance::prepare_nominal(db, &DefinitionHeaderKey)` returns a retained
`PreparedNominalVariances` with `pending()` and `variances() -> Result<&[Variance],
VariancePending>`. The tag79 `SourceVarianceQueries` owner calls it once and
retains its stable-encoded result. Actual Q74 field/schema receipts, Q76 lang
identity receipts, discovery catalog and solver configuration are retained;
nested fetches are dependencies of that memo. The producer accepts actual local
Struct/Enum/Union field closures only. Every admitted early Type/Region/Const
parameter is compared with the complete actual parameter at that index in
`cx.generics_of(current_item)`, protecting the compiler's indexed variables
against a foreign binder with the same index. Nominal arity is checked against
actual generic layouts. These are input identity checks, not a static checker.

`terms.rs`, `constraints.rs` and `solve.rs` are complete mechanically generated
upstream algorithm bodies with registered compiler-context/storage adaptations.
`tools/upstream_variance.py` verifies the exact originals and generated bodies.
The raw variance `mod.rs` is retained as query-domain provenance. Exact raw
`rustc_middle/src/ty/{util,generics}.rs` are registered for the separate query
domain's `opt_alias_variances` and `is_empty` contracts. No variance-vector
fallback, cap, unsafe code, independent fixed point or complete-facts provider
was added. The three compiler lang overrides use actual classified source IDs.

Separate source review against the eight recurring failure modes, at the file
fingerprints below:

1. `prepare_nominal` consumes retained field Types; it introduces no expression
   compiler or evaluator and does not execute source.
2. Both source origins enter `AdtSourceFields` and `SourceGenericLayouts`; no
   literal/break output path is present in these modules.
3. `require_supported` checks actual binder identities and shape only; all
   variance polarity, mutability and alias constraints remain pinned upstream.
4. There is no generated-source exclusion. Unsupported identity/type producers
   remain typed holds through the same source service in either origin.
5. No pattern/body lowering or checks of executable arms are requested. The
   demanded closed nominal dependency graph is the only discovery domain.
6. Field/source/config preparation is in one memo-owned cold plan; the pinned
   constraint walk reads immutable typed handles and invokes no source queries.
7. There is no namespace census in an algorithm callback or installed execution
   path. Dependency/currentness is owned by the source memo framework.
8. Retained `Arc<QueryValue>`/`Arc<InputValue>` roots share source metadata. Only
   field Type handles and compiler constraint/solution storage are newly owned;
   no BodyIndex is copied or whole-file node lookup introduced.

The independent source reviewer rechecked the actual locality closure,
SourceLangIdentity override receipts, Type/Region/Const namespace checks and
complete pinned walk; no blocking finding remained. This is a nominal producer
component review, not evidence of every retained compiler branch executing.
Function/constructor signature inference, opaque capture variances, foreign
variance inputs and Free-alias expansion remain explicit producer gaps; the
adapter rejects those missing inputs before entering their callbacks. Complete
DefinitionFacts/root admission, interpreter acceptance, JIT and broad runtime
matrices remain held. Pair's exact `[Covariant]` expectation is present in the
query-domain owner's unchanged-source host control, but was unexecuted at this
handoff. No Cargo or tests were run by this owner.

Completed audit terminals (all exit0):

* `/opt/homebrew/bin/python3.12 tools/upstream_variance.py`: complete pinned
  terms/constraint walk/fixed point matches registered adaptations.
* `/opt/homebrew/bin/python3.12 tools/bootstrap_trait_solver.py --verify-upstream`:
  pin `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, 227 files, 3,703,000 source
  bytes, exact upstream Git objects verified; retained originals changed0.
* `git diff --check` and Rust1.99 rustfmt of new modules.

Source SHA256 at freeze:

| Path | SHA256 |
| --- | --- |
| variance/mod.rs | `06703986ef3ebd7312e48e53fa8d7ca8d22b1eba863281840817a1d7e0d65081` |
| variance/adapter.rs | `765cda3a6317dcc91a8357b3677a820f93cf223b9e3c7baed46c81385f9f4d0f` |
| variance/terms.rs | `8fcebd0f35bb7b5858bc466953b9de525a14fd6498fd8d0a62a5ec81d5971ea3` |
| variance/constraints.rs | `749a1283b9cce47298381ed15475f94c52c493809a826bcbab7ec0e7df801657` |
| variance/solve.rs | `604b959be141f03fc33ba1763a5a330ce4ed5554612d8ebc9dcd392ae5dbac6e` |
| tools/upstream_variance.py | `9e3bf613576977e0f5a93db4c86daea1426bd7e376c1d1a456b3c503ccee8c96` |
| tools/bootstrap_trait_solver.py | `9740a0ad6217308ea055ddba8e573628c93377ec9e4c196d1c9526d7e03ac1eb` |
| vendor/rustc-trait-solver.files.json | `fc62eed92f5929709f75768b3ad03fb035ac8b04b77c86129d4217c4eb6abe84` |

## V502 mechanical repair and V503 freeze

The preceding table binds the initial source freeze, not the repaired candidate.
V502 compilation failed with six mechanical API errors. After all validation
processes were terminal and ownership returned, the bounded repair changed the
generator's two generated DefinitionId imports to read the actual parent import;
mapped the compiler Pattern spelling to `<Cx<'cx> as ty::Interner>::Pat`;
changed two Ty panic formatting sites from Display to Debug; and explicitly
imported `rustc_type_ir::inherent::Ty as _` in the handwritten entry module.
The generated modules were regenerated, not hand patched. Exact upstream
originals, the adapter's source-admission checks, solve.rs and provenance
registration were unchanged. This repairs API spellings only; it adds no
variance policy or accepted source domain.

Generator `--write`, scoped Rust1.99 rustfmt, generator audit and
`git diff --check` all returned exit0. No Cargo/tests were run by this owner.
V503 validation is separately owned and has no terminal evidence in this note.
The independent source review's semantic conclusions remain bounded to the
nominal-only producer and its explicit holds; compilation must be re-established
against the repaired source.

| Repaired path | SHA256 |
| --- | --- |
| variance/mod.rs | `6b207b82383b69633f4b401914cea061d444b26615f2086ff2b325b523801be6` |
| variance/terms.rs | `a8b2c7cb590fe1db734678039fd8f3d362282a506c692aee404fc48dfbacd460` |
| variance/constraints.rs | `e857eae0c312e844a5f9c770fa579615bf89b8ee471372f641ff7e6fe6127065` |
| tools/upstream_variance.py | `aca831a3ed7d249af9804140d81280424278d969bc2d35dfeb38faf3e5d9a4ca` |

## Read-only next prerequisite

Pinned `relate/solver_relating.rs::relate_ty_args` and the adapted
`infer/generalize.rs::relate_ty_args` fetch nominal variances only outside the
Invariant fast path. `relate.rs::AliasTy::relate` and
`outlives.rs::compute_alias_components_recursive` consult
`opt_alias_variances`; an ordinary projection has no variance vector, so its
arguments remain invariant. Opaque capture consumers in `opaque_ty.rs` and
`relate.rs::AliasTerm::relate` still need actual opaque identity/capture facts,
not nominal inference defaults.

The smallest useful next fact slice is actual trait-Self/associated-parent
predicate preparation. Q78 currently retains an explicit AssociatedContext
hold for implicit Self, inherited binders and associated declarations. Actual
source parent rebasing plus explicit associated bounds (including implicit
Sized) can feed pinned `collect/item_bounds.rs::{item_bounds,item_self_bounds,
item_non_self_bounds}` and existing `type_ir::elaborate::elaborate` with its
`filter_only_self` operation. Own clauses, inherited parent clauses and item
bounds are distinct query results and must not be populated from the same
convenient slice. Real consumers include solver `normalizes_to.rs:223,309`
(`own_clauses_of`), `assembly/mod.rs:869,885` (self/non-self item bounds),
`trait_goals.rs:106` (impl clauses), and adapted `infer/well_formed.rs:171`
(`clauses_of`). This requires source metadata, not typed function bodies or a
whole static checker.

The unchanged imported runtime witness
`tests/ui/associated-types/assoc-type-in-struct-constructor.rs` has
`Tr::T`, its `u8` implementation, `S<I: Tr>(I::T)` and the reached
`foo::<u8>(())` call. It needs no Box/GAT/opaque prerequisite. The negative
variance lifetime fixture is diagnostic/static evidence only; packed-field
Sized rejection likewise must not become a runtime static-checking refusal.
The proposed slice removes a true PredicateFacts hold, but does not alone
complete `PreparedProvider::publish/ready` or root invocation: genuine
DefinitionFacts/TraitFacts/ImplFacts, language registry, membership and
associated-leaf inputs remain independently required. No source writes,
Cargo or tests were performed for this investigation.

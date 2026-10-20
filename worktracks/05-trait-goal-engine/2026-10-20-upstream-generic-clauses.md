# M020 pinned generic-clause instantiation

Before-code bounded record, baseline `8c764548`. Owner: new
`traits/upstream/generic_clauses.rs`, `tools/upstream_generic_clauses.py` and this
record only. Exact pinned originals `rustc_middle/src/ty/generics.rs` and
`ty/mod.rs` are already retained and registered, so no new provenance inputs
are currently required. Parent owns registration and actual immutable source
clause bundles; the other frontend worker owns Q78/bound collection. No Cargo,
tests, commits, model/prepared/db edits or independent solver.

Invariant: own clauses and recursively inherited clauses are different source
queries. Parent-before-own order, spans and substitution must follow pinned
`GenericClauses` instantiation methods (`generics.rs:393-468`), using existing
`EarlyBinder`/ArgFolder. Actual source parameters include declaration owner IDs;
identity instantiation preserves parent parameter identities, while ordinary
instantiation into child identity arguments transports parent clause parameters
into the actual child's binder. Do not relabel inherited raw clauses as own,
flatten query distinctions, invent empty predicates or request live source
queries during this transport.

Primary unchanged witness is imported run-pass
`tests/ui/associated-types/assoc-type-in-struct-constructor.rs`: actual `Tr::T`,
impl `Tr for u8`, and `S<I: Tr>(I::T)`. Existing method-self-arg-trait and GAT
fixtures can provide associated parent-order controls. Metadata controls are
outside originals and do not claim runtime/full-fixture acceptance.

Shared entry extends the actual declaration clause bundle -> actual
SourceBindings/SourceGenericLayouts -> source configuration context -> pinned
GenericClauses instantiation -> existing shared Clause TypeIR. Immutable
authenticated parent records substitute for the compiler's `tcx.clauses_of`
callback. Both handwritten/generated syntax reach the ordinary declaration
source service. No executable preparation, alternative evaluator, custom
normalizer, query-kind merging or reflection-method port is introduced.

Cold preparation captures source clause and parent receipts once in the owning
memo. Instantiation walks only retained parent chains and clauses and allocates
the compiler-compatible result vectors; no parser, whole-file search, world
namespace census or dependency refresh occurs inside the pinned methods.
Authority/currentness is the owning memo's actual captured dependency closure.
Known gaps: complete explicit/own/all/item/super/implied predicate producers,
ready/full DefinitionFacts and runtime/JIT root admission remain independently
held. Closure requires mechanically audited complete method bodies, actual
source parent/child identity and order controls, independent source review and
the separate owner's compile/test terminals.

## Implemented interface and registered substitutions

`GenericClauses<'data,'cx,Q>` borrows actual source-bundle storage for `'data`
while Clause TypeIR belongs to `'cx`. Query markers `ExplicitOwnClauses` and
`FullClauses` cannot be interchanged. `ClauseParents<'data,'cx>` is a Copy view
over an immutable authenticated parent map with an associated query marker;
its `clauses_of` returns only that query kind. The actual source-bundle factory
alone supplies `from_source(parent, own_spanned_clauses)`. No Default is
implemented for this query payload, and no source callback or partial provider
is used to answer parent lookup.

Complete pinned methods retained: instantiate, instantiate_own,
instantiate_own_identity, instantiate_into, instantiate_identity and
instantiate_identity_into. The compatible InstantiatedClauses storage retains
both vectors, empty/is_empty/iter and both owned/borrowed IntoIterator bodies.
The reflection method after this block is deliberately outside the port.

Mechanical adaptations in `tools/upstream_generic_clauses.py`:

* Exact source checks for existing `generics.rs` SHA256
  `f5231c1921d2691ee02305a0c3987e420487c6b841be256c10eafe2752b935b0`
  and `ty/mod.rs` SHA256
  `bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7`.
* Context/concrete Clause/Args/Span and Unnormalized generic spellings;
  `'tcx` becomes `'cx`, with the separate source-storage borrow `'data`.
* pub(super) visibility and the typed query marker/constructor scaffolding.
* The four recursive/entry methods receive an immutable parent-map adapter;
  `tcx.clauses_of(parent)` becomes `parents.clauses_of(parent)` with the exact
  same parent-before-own traversal and argument slice. Tracing skips that view.
* The own iterator passes `args.items()` to the pinned binder iterator's slice
  API. Full instantiation still uses the original EarlyBinder/ArgFolder.

Identity results preserve each parent's source-owner parameter identity.
Calling instantiate with the actual child's identity Args uses the pinned
indexed substitution to transport inherited parameters into that child's real
generic table. The cfg helper `check_source_instantiation` takes genuine
source-owned views, actual args and independently source-encoded expected
identity/substituted clauses+spans from the parent's host harness; it checks
parent order, own suffix, both owned/borrowed iteration contracts and own
iterator exact-size/double-ended properties. It supplies no provider or clause
fixtures of its own.

## Separate source-aware review

1. No assertion/expression compiler or evaluator: only Clause TypeIR transport.
2. No literal/break generated-output path: both origins already joined through
   the ordinary clause producer before this interface.
3. No method/numeric/context policy: pinned EarlyBinder owns substitution.
4. No generated exclusion: the adapter accepts actual authenticated source
   handles regardless of source origin; missing frontend producers remain held.
5. No pattern or executable-arm work: only explicit own parent-chain inputs.
6. No parsing/planning callback: immutable map lookup replaces the compiler
   query; actual source preparation is memo-owned by the parent bundle.
7. No namespace-world scan/dispatch invalidation: exact prepared parent IDs
   are followed; currentness remains the enclosing bundle's dependency closure.
8. No BodyIndex copies or node searches: only compiler-compatible Clause
   handle/Span result vectors are allocated.

No unsafe, caps, fake complete facts, custom solver or normalizer was added.
The complete source-query distinctions and full root remain held. The host
helper is a source observation control, not full upstream fixture execution.
Generator --write and audit, scoped Rust1.99 rustfmt through generation, and
`git diff --check` returned exit0. Compilation/tests are unexecuted by this
owner and must be supplied by the separate validation owner after global freeze.

Source SHA256 before independent review: generic_clauses.rs
`a9a5b3cc7a7a87452072bdcea65b911095478f751a888ba73faa70618b5a1d87`;
upstream_generic_clauses.py
`326d36eabc92f135c956709e199f3a6379da0066e4f95219b7cdcf2cc005517b`.

Independent read-only review of these exact fingerprints subsequently found
all six pinned methods, full-argument parent recursion, order/spans, dual borrow
lifetimes, Args slice transport, Span folding wrappers and iterator contracts
preserved. The reviewer found no blocking mechanical/lifetime issue; this is a
source review only and does not substitute for compilation or test terminals.

## Integrated-control cleanup

The parent now directly exercises the production methods in
`source_declaration_clauses::check_actual_parent_source_clauses`, using the
actual encoded StreamEnumerate impl/GAT hierarchy. The unused generic
`check_source_instantiation` observation helper was therefore removed from both
generator and output; the earlier helper description above binds the initial
draft only. No production API or pinned method body changed. Generator
--write/audit, scoped rustfmt through generation and diffcheck returned exit0;
no Cargo/tests were run by this owner.

Read-only review of the integrated control found authentic parent/child
ParamKey checks, parent-before-own clauses/spans/order, identity preserving
parent owner IDs, pinned EarlyBinder rebasing into actual child identity Args,
own-iterator observations, no-query encoding and a retained complete-provider
hold. The immutable source hierarchy adapter keeps the ExplicitOwnClauses
marker. No fabricated provider, projection key or parent clause was introduced
in the inspected control. This is source review, not execution evidence.

Final cleanup source hashes: generic_clauses.rs
`2573862a2923d56b312a5cfcdb3ce14df46f2bd8eaa0434a05480e8b1fef8b43`;
upstream_generic_clauses.py
`483ca89ab031f0acbddc0f4aac0f6556f9931709737dea5276fba8e0d03f1092`.

## Read-only default-bounds / first-root prerequisite

Pinned `hir_ty_lowering/bounds.rs::{search_bounds_for,
collect_sizedness_bounds,add_implicit_sizedness_bounds}` considers actual
Sized/MetaSized/PointeeSized identities across direct bounds and same-parameter
where clauses. With no explicit sizedness, ordinary trait Self gets MetaSized;
parameters/associated Types/opaques get Sized. A lone relaxed/negative Sized
bound implies MetaSized if no other sizedness bound supplies it. Crate
`rustc_no_implicit_bounds`, PointeeSized's own trait definition and auto traits
have distinct skip rules. These are declaration collection semantics, not a
reason to add runtime static rejections.

Current `frontend/language_items::sized` resolves only the actual configured
Sized declaration. `facade/core/marker.rs` has Sized but no MetaSized or
PointeeSized declarations. Recognizing their compiler-role strings in Q76 is
not source evidence for missing declarations. Consequently simple Q78 Sized
recipes cannot yet certify complete generic/trait explicit compiler clauses;
the common missing prerequisite is the actual sizedness hierarchy and a pinned
collector adapter, not additional names in fixture-specific recipes.

Other default traits are separate: pinned `ty/context.rs::default_traits`
returns DefaultTrait1..4 only with `-Zexperimental-default-bounds`; without that
actual compiler option it returns an empty list. The current standalone source
configuration has no -Z override input and does not yet capture
rustc_no_implicit_bounds. A production completion proof must establish those
configuration/attribute inputs or retain their explicit hold, never silently
treat unsupported profiles as empty defaults.

The shortest declaration-only full-clause role is an ordinary free function.
Pinned `outlives/mod.rs::inferred_outlives_of` returns no inferred clauses for
Fn, and `collect/clauses_of.rs::clauses_of` adds Self:Trait only for traits.
Thus a free function's full clauses really equal its complete explicit clauses;
parentless Fn with no generics/bounds has established empty clauses, without
running outlives inference on fields or checking a body. This proof does not
apply to generic nominals, checked aliases, traits or associated item bounds.
Item/super/implied-bound query applicability must remain distinct; impossible
queries must not be filled with convenient empty vectors.

Smallest unchanged imported declaration witness is
`tests/ui/generics/generic-derived-type.rs::main`, whose declared signature is
fn()->() and has no generics/bounds. A source-derived WellFormed(FnDef(main,[]))
root needs the actual receipt, zero generic table/variance result, declared
FnDef/signature Types, complete role-certified clauses and real language/fact
registry publication. Pinned `infer/well_formed.rs` visits the return Type and
then nominal clauses for FnDef; unit adds no further Type dependency. No body
lowering, interpretation or whole-body static checking is needed to prepare
that invocation. This is a proposed first genuine solver-root component, not
runtime acceptance or permission to bypass existing publish/ready gates. The
same fixture's g<X> or f<T:Clone> needs actual generic sizedness and trait facts
and should remain held until that common prerequisite exists. Investigation
was read-only; no source edits, Cargo or tests were performed.

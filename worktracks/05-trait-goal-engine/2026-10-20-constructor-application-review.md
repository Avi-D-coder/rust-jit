# LOCAL-049 constructor application transport: independent source review

Read-only review of the lead's mutable constructor patch, baseline
`0c11e2e3aaa11513eb8860875e97c329dc23bbb9`. No source edits, Cargo, tests or
runtime invocations by this reviewer. The actual V505 unchanged witness
`associated-types/assoc-type-in-struct-constructor.rs` failed at reached
`foo` -> `S::<I>(i)` before compound field binding; this review does not turn
that failure into an execution pass.

The inspected call patch splits source declaration classification from actual
application binding. `NominalRunFacts.calls` is keyed by BodyKey/CallIdentity
and stores a NominalConstructorTarget or the established non-constructor
classification. `call_bindings` additionally keys the bound result source
context and complete Option<Vec<Option<TyTerm>>> of explicit source arguments.
Frame::call_identity preserves source node and runtime-call ordinal.
OwnerArguments::source_terms uses the existing Frame::bound_view, retaining
source identity while transporting this activation's actual generic binding.
No descriptor equality is used to choose an associated implementation.

`bind_constructor_application` consumes explicit all-written source terms
before field binding via the existing NominalMember query; its result retains
the PreparedInputGuard, checked before frame installation. Positive cached
OwnerArguments revalidate selected Type/source-plan guards. Target/negative
classification stays within the run's existing source-publication/body lease
boundary; it is not exported as an independently current semantic witness.
The run rejects a changed publication before reuse. No new world census was
introduced by this patch; the pre-existing run-wide guard scope is not claimed
to satisfy unrelated-invalidation performance acceptance.

Pattern isolation is sound for the inspected change:
run_pattern_names::pattern_constructor calls bind_selected_constructor, which
passes explicit None to the application helper. The helper never invokes
constructor_arguments itself, so an enclosing expression's turbofish cannot
leak into a pattern merely because both share constructor field binding.
Lexical target classification in run_value_items still precedes module lookup
and distinguishes enum associated functions from actual declared variants.

Two explicitly scoped corner cases were reported to the lead:

* Frame::bound_view currently descends Param/Ref/Slice/Array/Tuple/Nominal, not
  Projection/Function/Dyn and every other Type family. An explicit `S::<T::A>`
  must keep its source producer pending unless the common substitution closes
  it; the cache cannot pretend an unchanged symbolic term is a fully bound
  source application across T activations. Existing explicit-template admission
  and NominalMember pending boundaries were inspected; no descriptor-erasure
  bypass or successful bogus projected witness was found.
* One explicit hole makes the binder fall back to the whole contextual argument
  list. It does not invent a Type for `_`, and the cache keeps the exact partial
  vector. With no independent result context, partial known arguments cannot
  currently resolve a compound field. That remains a named producer hold;
  future partial transport must preserve known positions and actual holes.

Admitted path arguments are selected from the retained SourceLocalPlan once
per exact PC/position, with guard checks on reuse. Per-activation source
substitution allocates/clones Type terms and compares the full cache key:
O(explicit/result Type representation size), not repeated syntax parsing or
field-plan preparation. Cache misses demand one memoized NominalMember
specialization. Legacy no-BodyEnvironment signature parsing remains held
migration debt, unchanged by the split. No performance measurement is claimed.

All-eight source review: (1) no assertion/expression evaluator added; (2) no
literal/break-only generated route; (3) no method/numeric identity rule;
(4) shared source application route rather than generated exclusion; (5)
patterns isolated and no cold executable body forced; (6) retained Type plan
and memoized field binding, only actual activation substitution repeated; (7)
existing exact guards/source-publication boundary, no added namespace census;
(8) Arc bound variants retained, no BodyIndex deep copy or whole-file node
search. No unsafe, cap, source pending suppression, manufactured projection or
independent solver was introduced in the inspected delta. Historical MA001–008
removal/acceptance obligations remain separate and held.

No blocking issue was found in all-written call argument transport at this
snapshot. The lead confirmed unit-constructor value entry remains the existing
context-only wrapper/per-PC value cache; this slice makes no new explicit-value
argument transport or value-cache acceptance claim. Partial holes and the
incomplete Projection substitution traversal are likewise unchanged held scope;
unsupported explicit projections cannot become completed proof. Pattern
binding continues to pass explicit None. Hole/projection, all-edition/origin,
exact-once, currentness and real fixture execution remain validation requirements.

Inspected snapshot SHA256:

| Path | SHA256 |
| --- | --- |
| engine/run_nominal.rs | `9c9c787c66ede0322ef374c4b7476b2bc56bf6935157e8a369b5a026459b3edd` |
| engine/run_owner_arguments.rs | `4bf483827d37f89a7038aa0b2c4d06de4901df5629bcf479d7af3e0b8884ad67` |
| engine/run_value_items.rs | `3cf882ba0f800e5b9cfdd7f0e49daced43f56276d1aa574ddbf9684ff8682ff3` |
| vm/frame.rs | `6e1637d5a3217577c78217fa719136b3c5eb4e96fa4847963246b7d8ab3cb997` |
| engine/run_pattern_names.rs | `e128e63fecd8a39910e1f5ee5a05c47eff2231f495491e06639326b629b027c1` |

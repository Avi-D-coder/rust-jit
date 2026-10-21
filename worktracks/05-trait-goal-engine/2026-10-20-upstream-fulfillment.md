# M019/M020 / LOCAL-049: pinned multi-root fulfillment component

Preimplementation source review, baseline `7ea94b8f6b3ff5b9a0793a414e2040aa2828d0ff`.
This bounded assignment owns new `infer/fulfillment.rs` and a new reproducible
projection tool only. The lead owns module registration, manifest/provenance
registration, actual source-fact admission, root ownership, integration and tests.

The invariant is that mutually constraining demanded roots share the same actual
upstream inference owner and rustc fulfillment policy. Evaluating separate roots
once, writing an LRR retry scheduler, or replacing them with a synthetic
`WellFormed` root would violate it. The minimal witness is two demanded roots
where the second assigns a variable on which the first root has stalled. The
unchanged upstream semantic witness is pinned
`compiler/rustc_trait_selection/src/solve/fulfill.rs`, SHA256
`b847f09571a9b182c1456ea0c3fc9ccae43457ac7b12f01f1d28c0573e06ad7f`,
at Rust revision `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
Applicable unchanged language fixtures and execution remain the lead's acceptance
work; this component alone supplies no language-fixture pass.

The common owner already exists: `infer::delegate::SolverContext` owns one
`InferenceContext` and dereferences to it. The pinned
`SolverDelegateEvalExt::{evaluate_root_goal,goal_remains_stalled}` blanket
implementation already accepts that owner. `InferenceStorage::num_open_snapshots`,
`InferenceContext::{shallow_resolve,sub_unification_table_root_var}` and the real
`Interner::recursion_limit` require no new accessors. The actual shared crate's
`solve::fast_path::compute_goal_fast_path` supplies registration. Keep its stalled
state and its root-resolution update; do not duplicate its canonicalization,
candidate transactions, projection or search graph. Source facts enter through
the immutable prepared provider and must already be admitted. Missing source is
not semantic failure. Arbitrary replacement-goal admission is not authorized.

The port will keep generic caller cause metadata, the actual predicate/ParamEnv,
origin span and recursion depth. Compiler-only diagnostic derivation, HIR/MIR
region-dependent replay, and compiler coroutine-typeck drain are outside this
interpreter component. Retain typed original-obligation NoSolution and ambiguity
outcomes. Pinned fulfillment progress overflow is fatal at exactly rustc's
configured recursion comparison; translate the diverging diagnostic into a typed
terminal outcome and prevent further processing. No independent work/count cap
is introduced. Preserve upstream `MaybeInfo`, including search overflow, through
the stalled result rather than promoting it to success or NoSolution.

The ordinary frontend/member consumer must supply demanded roots from the same
source/type service for handwritten, arbitrary user-macro, wrapper and standard
macro nodes. This component parses, lowers and executes no source, never examines
cold bodies and owns no guest continuation. The actual source/root/VM join,
dictionary witness extraction, and legacy semantic route removal remain held.
Pending roots and their inference stalls are local to one snapshot. Registration
is a fast-path computation; fulfillment has pinned `ThinVec::retain_mut` passes
until the pinned progress criterion stops. No global namespace scan, plan
reconstruction or immutable body copy is added. No speed claim follows.

All eight architecture modes reviewed before implementation: (1) no assertion
evaluator; (2) no macro-output executor; (3) no macro-specific member or numeric
rules; (4) no generated-source gate; (5) no pattern engine or cold-arm executable
check; (6) no parsing/planning callback; (7) no namespace map/scanning or cache
invalidation code; (8) no BodyIndex ownership/search. This is a bounded source
review of the new adapter, not removal of MA001-MA008 or acceptance of neighboring
production paths. After generation record exact new-file fingerprints and a
separate source review. Runtime, full fixture, milestone, interpreter, JIT,
performance, and Linux gates remain open.

## Generated source review and handoff

Creation was released after source checkpoint `edd7f85`. The generated source is
`crates/lrr/src/traits/upstream/infer/fulfillment.rs`, SHA256
`03ff24c31c25e9a5e04a675296cc4afbb06b5068db5784b0b6659b9a4895fc9d`.
Its reproducer/auditor is `tools/upstream_fulfillment.py`, SHA256
`a199b46f7dc7ab0f93a1b635c1e61129c03c4dfce8d6cd3430101be683f58758`.
The tool checks exact original hashes before selecting methods and emits the
whole port through Rust1.99.0 rustfmt. `python3 tools/upstream_fulfillment.py
--write` and then `python3 tools/upstream_fulfillment.py` both exit0. This is a
projection/provenance/syntax check, not Rust type checking or executed semantics.
No Cargo operation or language control was run by this contributor.

New pristine inputs are the pinned Git object bytes, with the existing vendor
MIT and Apache licenses applying:

| Original | SHA256 |
| --- | --- |
| `compiler/rustc_trait_selection/src/solve/fulfill.rs` | `b847f09571a9b182c1456ea0c3fc9ccae43457ac7b12f01f1d28c0573e06ad7f` |
| `compiler/rustc_infer/src/traits/mod.rs` | `4bfd95cdd4b0e3b18cdc0a60ae81470d916556d2270fe9a1556d31206a03757c` |
| `compiler/rustc_structures/src/limit.rs` | `aaed7ebb13ff9ba166be0ef015e86f9bdfff8f1cba9e4e15a950a96932ae9faf` |

`FulfillmentContext<'a,'db,C:Clone>::new(&'a SolverContext<'db>)` binds exactly one
owner. Its `PredicateObligation` retains generic caller cause C plus actual span,
ParamEnv, predicate and recursion depth. `register_predicate_obligation` runs the
actual shared fast path. `try_evaluate_obligations` and
`try_evaluate_obligations_with_inspector` return
`Result<ThinVec<NextSolverError>,FulfillmentOverflow>`; the inspector occupies
rustc's evaluated-obligation inspection point and adds no reevaluation.
`collect_remaining_errors` drains pending entries as typed ambiguity and preserves
their actual stalled state. `pending_obligations` and the pinned sub-root/float
filters expose original obligations. All snapshot/delegate/type-variable APIs
already exist; no new shared owner accessor was needed.

Separate source pass compared registration and the progress loop to the exact
original: snapshot-count assertions precede source-goal work; stalled entries
use `goal_remains_stalled`; real `evaluate_root_goal` resolves the saved predicate;
only `HasChanged::Yes` advances an obligation's depth or another pass. The
comparison is `depth > configured_limit`, the negation of pinned `Limit`'s
`value <= self.0`. A fatal progress overflow retains the remaining entries and
the earlier TrueErrors, returns immediately after `retain_mut`, and terminally
stores the result. Further evaluation/remaining-error collection returns that
same terminal outcome; registration refuses resumption. No later root gets
evaluated after the abort. The pending containers and returned obligation list
retain upstream ThinVec. The source pass corrected mechanical selection of a
same-named storage helper and the generic cause/span representation before
freezing these fingerprints; no build result is attached to earlier drafts.

All eight required modes were inspected again against these fingerprints:

1. `PredicateObligation::as_goal`, registration and evaluation have no expression
   compiler/evaluator or assertion identity. MA001/MA002/MA003 are not extended.
2. No parsing/output execution API occurs in the new module; MA004/MA005 remain
   outside this slice and held.
3. Fast-path, inference, member and numeric semantics come from the same
   `SolverContext` and upstream library; no macro/name switch is added. MA006
   and MA007 are not extended.
4. The port receives typed actual goals and has no original/generated-source
   branch or new source-origin refusal. Actual source hygiene remains the
   lead-owned preparation gate.
5. No pattern operation, body preparation, HIR type checking or cold executable
   arm traversal exists. The compiler's HIR/MIR region-dependent-goal replay and
   coroutine Typeck-specific drain are deliberately not projected.
6. Registration evaluates only the upstream fast path; the pinned loop uses
   retained typed roots and stall state. No parser/planner enters execution.
   Source/type callbacks must consume the lead's admitted immutable facts.
7. No namespace structure, relevant-world census, invalidation scan or alternate
   cache is introduced. Upstream root evaluation owns its normal search graph
   and canonical dependencies; source dependency admission remains external.
8. No BodyIndex, source-file traversal or immutable body clone is introduced.
   Error/pending clones copy obligation metadata/retained typed handles only.

No obsolete production route is removed by this bounded port. MA008's old
index/verifier paths, historical custom trait routes, complete actual-fact/root
activation, selected witness transport and lazy dictionary/VM integration stay
explicitly held. The lead must register the module, standalone generator audit,
all three original inputs and their manifest/provenance rows before integration;
then build and exercise source-authenticated multiple roots. The unchanged
upstream `coercion/coerce-depth.rs` and
`overflow/dont-drop-obligations-on-overflow-{1,2}.rs` record relevant pinned
progress/abort contracts, but are unexecuted here. A source-only reusable port
does not complete M020 or any runtime/full-fixture/milestone/interpreter/JIT gate.

## Registration follow-up

The lead subsequently transferred the bounded registration files. Registration
now adds `pub(crate) mod fulfillment` to `infer/mod.rs` and the same exact output
to `tools/sync_upstream_inference.py`, which calls the new generator's `expected`
function. This is the existing inference projection/audit owner; bootstrap audits
the pristine vendor tree, while the inference audit also checks the generated
port. `tools/check_source_checkpoint.py` is staged artifact hygiene, with no
generator audit registry, so it needed no change. `vendor/UPSTREAM.toml` has no
trait-solver section and needed no change. `tools/bootstrap_trait_solver.py`
and `vendor/rustc-trait-solver.files.json` now register exactly the three pinned
inputs above in the sorted support closure, with byte counts, upstream paths
and identical original/installed SHA256 values. No existing algorithm output or
pristine input was rewritten.

Actual terminal evidence, no Cargo or Rust test operation:

- Initial `python3 tools/sync_upstream_inference.py` exited69 because the system
  Python launcher requested Xcode license agreement. No agreement was attempted.
- `/opt/homebrew/bin/python3.12 tools/upstream_fulfillment.py` exited0.
- `/opt/homebrew/bin/python3.12 tools/sync_upstream_inference.py` exited0, including
  the registered fulfillment module and all existing inference projections.
- `/opt/homebrew/bin/python3.12 tools/bootstrap_trait_solver.py` exited0:
  pin `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`,9crates,238files,
  4197357source bytes,0retained-original Rust changes. Its existing14registered
  stable Rust adaptations remain separately reported; the default command does
  not perform new upstream Git-object verification. The three new originals
  were originally copied directly from the exact pinned Git objects.
- The bounded registration diff check exited0. Independent read-only review by
  `/root/validation_review` also confirmed pinned progress/stall/snapshot/overflow
  alignment and audited the same fulfillment fingerprint; no Cargo pass follows.

Registration source fingerprints:

| File | SHA256 |
| --- | --- |
| `infer/fulfillment.rs` | `03ff24c31c25e9a5e04a675296cc4afbb06b5068db5784b0b6659b9a4895fc9d` |
| `infer/mod.rs` | `a3cba587bb60a8b53b50b8f65cb00922aa1aa0842fcf2e9b071940003430e75d` |
| `tools/upstream_fulfillment.py` | `a199b46f7dc7ab0f93a1b635c1e61129c03c4dfce8d6cd3430101be683f58758` |
| `tools/sync_upstream_inference.py` | `679ee0eed479e01145544bcb0d2625f1cfc386f71e2bf5eb398e09ed4a9e3c0e` |
| `tools/bootstrap_trait_solver.py` | `8e76124b9ffb8189c2705ca5e11c3bca2506c140ebee3761350923e3b4084b1f` |
| `vendor/rustc-trait-solver.files.json` | `2d3859931eb1709342f1391ce6df8601cf2635bfff39dfebddc7bcfd87472f5e` |

The earlier handoff's pending registration requirement is satisfied by these
source audits. Actual source-factory/root entry, Rust compilation, interpreter
controls, accepted selected-impl evidence and all previously listed gates remain
held until the lead's combined validation records actual terminals.

# M020 — reuse pinned transitive relation for placeholder assumptions

Solo root / LOCAL-049 / WT05-02; baseline a55023600e0eade46616011d0170cfead210e7b9.
E-R5-014 and specification 53.0 require actual upstream inference, including
placeholder assumptions, before a real delegate/root call. Minimal source witness:
`Assumptions::<Cx>::empty()` reaches the stable TransitiveRelationBuilder::default
panic stub in rustc_type_ir/src/region_constraint.rs. Reproduce under the actual
existing source-context matrix before replacing the stub. Upstream language
witnesses remain methods/method-two-trait-defer-resolution-2.rs and
traits/multidispatch1.rs; their runtime contracts are not passed by this component.

Extend the existing shared rustc_type_ir region_constraint API, never a local
assumptions graph. Retain byte-identical pinned compiler frozen.rs,
transitive_relation.rs and its entire unchanged test module. Compile the exact
helper with mechanical module/hash-alias import changes only. Replace the stable
panic-stub declarations with that helper's exports; preserve original files as
.upstream and register deterministic, hash-pinned build projections. Bootstrap
import and audit must both reproduce/check those projections; no arbitrary
edited-vendor-source allowance. Nightly imports are unchanged. No new dependency,
algorithm, solver, evaluator, unsafe, limit, accounting or static checking.

Source route: admitted fixture source/Q69-Q70 and actual source_terms Encoder ->
actual typed Cx -> existing check_actual_storage host control. Original/user emit/
wrapper and all4editions share this route; no macro/source-name policy. The
placeholder relations operate on retained interned Regions, with no IO, query,
namespace census, body parsing/lowering or installed VM changes. Construction is
cold inference work: preserve actual upstream BitMatrix closure and its cost;
no performance claim. Existing immutable dependency/currentness owners remain.

First acceptance: observed stable-stub failure; unchanged18upstream relation
component tests; actual empty/nonempty/directional/transitive/inverse/clone
assumptions controls with real source Cx and existing source/order/cleanup matrix;
source/HEAD/binary guards and async macOS1.99.0 terminal evidence. Host IR graph
controls are supplemental, not invented language fixtures or stock-main passes.
Record a separate source review of all8 recurring failure modes and the distinct
MA findings. Full InferCtxtLike, region constraint rollback/visibility/leak checks,
forall/relations, full source facts, real delegate/root, selected member/runtime
migration and legacy removal remain explicit prerequisites; do not close M020,
interpreter, runtime, JIT, Linux or performance gates.

The slice also transports the exact compiler insert/get/has-placeholder-assumptions
methods through the existing registered inference context port. InferenceContext
owns the same RefCell<IndexMap<UniverseIndex,Assumptions>> as pinned InferCtxt;
initially empty, no made-up root entry. Source inspection confirms pinned rustc
snapshot rollback restores universe but does not snapshot this map (compiler
mod.rs FIXME at the field). Preserve that behavior and keep full forall/region
transaction acceptance open; this slice does not claim assumptions rollback.
Missing get entries retain the upstream invariant failure, never an empty-success
fallback. Existing TypeVisitable escaping-bound invariant checks are retained;
these check compiler IR well-formedness, not guest static borrow checking.

## Reproduce the shared helper independently

From this checkout, first `python3.12 tools/bootstrap_trait_solver.py --verify-upstream`
(optional checkout required only for Git-object comparison). With sources frozen,
run asynchronously using the same single Cargo owner:

```sh
cargo +1.99.0 test --offline --locked -p rustc_type_ir --lib data_structures::transitive_relation::tests:: -- --nocapture --test-threads=1
```

This selects 18 unchanged compiler helper test bodies, not rustc runtime fixture
mains. Both original and compiled test modules have SHA256
`a0b9f92d4c889a033a915cbd6d83b5b8a6e87555a3bcd19a43db032c49dabc74`.
Their direct inclusion also exercises the helper's private graph/lattice code.
The existing actual source-context matrix is selected separately in the tracked
region-assumptions-controls.json; follow tools/validation/README.md with a new
output directory and actual Cargo JSON selected binaries. Old artifacts are not
inputs. The V342 pre-repair failure is reproduced by calling
`rustc_type_ir::region_constraint::Assumptions::<DbInterner>::empty()` in the
actual source-context control on baseline a550236; it panics at the stable stub.

Full fact publication, source function/binder metadata, solver-region state and
undo integration, compiler-compatible region visibility/leak rules, generalizer/
relations/canonical responses, real delegate/root and source-selected runtime
migration remain the next prerequisites. Do not implement their callbacks using
empty/default successes to get an artificial first solver answer.

Final V343 library74043, helper47213, CLI2624 and controls32307 all terminal exit0.18/18unchanged upstream relation tests and5/5source-context controls pass; source/HEAD/test/CLI guards match. Fresh-import/audit control48724 exits0. V342 library15301 exits0 and control86571 exits1 at the reproduced stable stub; all failed evidence retained separately. The reviewed final production source did not change after validation. No actual root invocation or new stock runtime pass follows. Full gates remain held. Exact commands, hashes, scope and terminal records: worktracks/evidence/2026-10-19-v342-v343-upstream-region-assumptions.json.

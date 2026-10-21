# Interpreter handoff — October 20/21, 2026, round 2

Continues [the winddown handoff](2026-10-20-interpreter-winddown.md). Repository `/Users/avi/Code/rust-jit`, branch `codex/lrr-implementation`. Read `AGENTS.md` and the revision-5 specification first. The project goal (interpreter first, tested against unchanged upstream rustc programs, then the JIT) is **incomplete**; the JIT gates are unchanged.

## Actual acceptance, not component counts

[V540](../../worktracks/evidence/2026-10-20-v540-interpreter-runtime.json): 71 unchanged official rustc programs through the production CLI/VM with the maintained `tools/rustc_checkout_runtime.py` (two invocations; the harness caps a run at 64 cases): **70 `runtime_contract_pass`, 1 `unsupported_harness`** (`closures/2229_closure_analysis/migrations/insignificant_drop.rs` carries the unhandled `//@ run-rustfix` directive, so it was not run), 0 interpreter failures. Tested source fingerprint `2f881ff698b772cec4ce576f6e6dd90333b86af3d485c08634845c3d63efe282` (HEAD `ba8e4ab34c` plus the later-committed `engine/run_capture.rs`); before/after-build/after-runtime guards agree. This is a selected batch, not a suite total, full-fixture, diagnostic, tooling, complete-interpreter or JIT result. The tracked selection is `worktracks/05-trait-goal-engine/2026-10-20-interpreter-runtime-selection-round2.json`.

The two programs named in the previous handoff now pass:

- `tests/ui/enum-discriminant/repr128.rs` — the reached call now uses the pinned next-generation solver root (relevant-impl dependency preparation), and an operator on a rigid operand dispatches its lang-item trait method.
- `tests/ui/union/union-generic-rpass.rs` — nested `Iter<_>` hole inference through the same pinned root.

Only these two programs reach the solver root at runtime (one catalog-discovery suspension each); the other 69 do not. Failed intermediate attempts are in the evidence file (V538: harness case cap; V539: those two programs completed but the capture reported a suspension, see "Capture classification").

Separately, an unchanged-program survey of 460 sampled plain run-pass programs (exit 0 and `completed` only; no stdout comparison; an earlier binary of this round) gave 299 pass / 161 fail. It is a triage sample, not a count, and its failure clusters drove this round.

## Changes this round (commits `2ac4f394ac`..`3b1263ff86`)

Each item names the shared entry point extended. All were driven by unchanged upstream programs.

| Area | Shared route extended | Driver |
| --- | --- | --- |
| Reached-call callback dependencies | `traits/upstream/relevant_candidates.rs` (pinned `NormalizationFolder` + `for_each_relevant_source_impl`), retry loop in `reached_call.rs` | repr128 |
| Rigid operand operators | `vm/frame_operator_calls.rs::overloaded_operand` keys on the operand's static source (Param/Projection/Opaque), `engine/run_operator_calls.rs` binds the argument view | repr128, generic-extern-mangle |
| Primitive operator impls | `tools/gen_core_ops_impls.py` -> `facade/core/ops_impls.rs` (mixed-RHS shift matrix omitted by default; `--all-shift-rhs`) | ret-associated-type, generic-extern-mangle |
| Thin references in union/transmute byte images | `runtime/cell_layout.rs` plain images, `cell_pointers.rs::pointer_from_address`, union pointer slots | union-nonzero |
| Qualified pattern paths (3+ segments) | `frontend/source_value_goals.rs` pattern branch -> module path route | functional-struct-upd, derive(Debug) |
| Statement macros with inert attributes | `lrr-expand-adapter/src/syntax_attachment*`, cfg-disabled subtree skipping in `declaration_scan.rs` | enum-discrim-autosizing |
| Region arguments in impl heads | `frontend/associated_types.rs::bind` | Ref/RefMut/user guards |
| Compiler-owned def-site pattern bindings | `frontend/macro_definition_modules.rs`, `source_value_goals.rs` | derive(PartialEq) enums, small-enums-with-fields |
| Open constructor arguments | `ExplicitType::Open`, `engine/run_constructor_inputs.rs`, `ReachedSubject::Adt`, `runtime/cell_open_construction.rs`, `traits/upstream/source_impl_associated.rs` | union-generic-rpass |
| User `Fn*` impls via call syntax | `Op::CalleeDeref` classification, `OverloadedCall` in operator route | overloaded-calls-*, unboxed-closures-* |
| Sized `rest @ ..` bindings | `Projection::Subarray` inside the one pattern transaction | zero_sized_subslice_match |
| Open generic call results | `vm/context.rs::declared_result_admitted`, `frontend/call_contracts.rs` | rcvr-borrowed-to-slice |
| Facade | iterator adapters, Range/ptr/unit ordering, Arc/Weak, Mutex/RwLock, ThinBox, `std::alloc`, `os::raw`, escapes | Arc/inspect/skip_while programs |
| Capture classification | `engine/run_capture.rs`: `InputRequest::Discover` is counted as host preparation (`host_preparation_suspensions`), all other suspensions still latch | harness |

### Capture classification (decision to review)

A reached solver root suspends once for the actual impl catalog and the host services it through ordinary compiler/module preparation, then resumes the same interpreter. The runtime capture previously latched any suspension as an unobserved boundary, so repr128 and union-generic-rpass completed `main` yet reported `incomplete_evidence` (V539). The capture scope is "dispatcher external effects"; host preparation streams are documented as outside guest capture, so a `Discover` suspension is now counted instead of latched. Macro expansion, include and module-capture suspensions remain latching. Revisit this if catalog demand ever becomes visible to guest behavior.

### Other decisions to review

- **Regions.** Runtime-derived source terms carry no region arguments; `source_terms.rs::complete_erased_regions` completes missing slots with `'static` in root encoding. This follows the repo's erasure convention and "looser than rustc" (no borrow checker); it would be wrong if region constraints ever become observable.
- **Legacy normalizer still primary** for projection-typed ADT field types (`engine/run_nominal.rs`); the pinned route runs when the legacy one reports the pending-projection error. This is migration debt, not the target design.
- **`db/prepared_inputs.rs::prepare_input_guard`** now records the guard's input stamps as dependencies of the embedding provider (needed so recipes installed before a mid-run catalog publication renew). Reviewed as additive-only; no performance measurement.
- **Primitive operator impls** add startup work (hello-world user CPU about 5.4s debug); the full mixed-RHS shift matrix was measured at about +2.4s and omitted. A generic `T: Shl<u32>` instantiated at `u8` is therefore ambiguous.

## Next work, in order of value

1. **`assert_eq!`/`assert_ne!` failure path.** A failing assert reports `LRR-E-TYPE-CONTEXT: generic unit variant requires its type context` instead of panicking (native `assert_failed` in `facade/functions.rs` declares no parameters, so its `None` argument has no hint). Independent and small.
2. **Source-component root for deferred inference** (largest family, ~16% of sampled failures). Design from this round: a retained `SourceRelationPlan` feeds a same-owner upstream root with rustc's fallback phase only after closure; keep `binding_constraints.rs` component discovery, move its decisions to the pinned solver; poll at the leaf's first reach like `settle_open_constructor`; delete the legacy Via/Step branches as each family lands. Reject more runtime fallbacks, widening `LiteralRecord` to open `None`/`Vec::new()`, and macro-named cases. Slice 1 acceptance: `match/guard-pattern-ordering-14865.rs`; slice 2: `overloaded/overloaded-deref.rs`, `overloaded-autoderef.rs`, `deref/deref-mut-closure-drop-order.rs`. Closure terms and `Fn*` builtin callbacks are needed for `Map<I,F>::next` (every exhausted `.map(f)` currently fails).
3. **Float parsing.** `impl FromStr for f32/f64` needs a correctly rounding host intrinsic (`numbers-arithmetic/f32-literal-rounding-32805.rs`).
4. **Qualified-path and operator gaps:** `<i32 as Mul>::mul(a, b)` is ambiguous across the trait's argument impls (`frontend/callables.rs` ~1186 picks by self type and trait only); `a + b` with `&'a T: Add<&'a T>` where-clause reports `GENERIC-BINDING builtin operator result conflicts`; method calls on `()` receivers; block-local module roots with 3+ segment paths; variants through type aliases and `Self::Variant` constructors.
5. **Library/interpreter gaps seen in programs:** closure-returning adapters through `collect()`, `partition`/`unzip` generic returns, `bytes().collect::<Vec<u8>>()`, `*rest` value read/assign for array rest bindings, constant promotion of `&[N(1), ..]`, cfg on call arguments, `#[cfg(any())] let` inside a callee, fat references and typed punning in union images, `slice::from_raw_parts_mut`.
6. **Harness:** `//@ run-rustfix` is unhandled; the 64-case cap and the capture rules live in `tools/rustc_checkout_runtime.py` and `engine/run_capture.rs`.

## Resume workflow

Unchanged from the winddown handoff, plus: `tools/validation/regress_retained.sh [binary]` is a fast PASS/FAIL screen over 32 retained programs (exit 0 and `completed` only; not acceptance). Use `tools/rustc_checkout_runtime.py` for acceptance, in batches of at most 64 cases, with `source_manifest.py` before/after-build/after-runtime. Keep one Cargo owner; give other agents disjoint files or read-only analysis, because the facade is parsed at runtime from the embedded copy and a broken facade file breaks every program run after the next build. Filtered `cargo test` runs this round showed pre-existing failures in `operator_dispatch_tests`, `numeric_constraint_facade_tests`, `generic_boundary_tests`, `trait_header_tests`, `test_catalog_primitive_namespace_tests`, several `pattern`/`closure` filters and a stack overflow at the default test stack (use `RUST_MIN_STACK=268435456`); those are internal checks, not acceptance, and were not triaged individually.

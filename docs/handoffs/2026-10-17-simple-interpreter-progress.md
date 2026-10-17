# Simple-interpreter progress — October 16–17, 2026

Continues [the October 16 handoff](2026-10-16-interpreter-handoff.md) under the user's new direction, recorded as [E-R5-006](../errata/revision5-no-resource-accounting.md) and at the top of `AGENTS.md`:

- Allow only limits that rustc itself enforces.
- No sandbox, IO-permission or capability work for now.
- Edit and build directly in this checkout.

Nothing here is committed. The working tree also still carries the inherited uncommitted WIP listed in the earlier handoff.

## Baseline facts established

- **Clean HEAD (`edd99619e3`)**, built in an isolated worktree: the lib test suite has 1,483 tests, with 1,389 passing and 94 failing. The earlier handoff reported only focused checks, so these failures were pre-existing.
- **Main checkout with inherited WIP** needed two compile fixes:
  - wired up `engine/run_module_source_preparation.rs`;
  - changed `trait_headers.rs` to encode `Key` with `e.bytes(&key.0)`.
- **After those fixes and the changes below** (before the ceiling sweep): 1,495 tests, 1,401 pass and 94 fail. There are no new failures against HEAD; the two extra failures belong to new inherited-WIP tests.

## Language/frontend changes (common mechanisms, no context-specific routes)

| Change | Where | Why |
| --- | --- | --- |
| Configured source authority root | `SourceMacroExportFacet::capture_in_source_root`, `cli/macro_export_modules.rs`, `source/macro_export_module.rs` | `core`'s `#[path = "../../portable-simd/…"] mod core_simd;` reads outside the package. The coordinator grants the library tree, which contains the package. The default (package root) is unchanged. |
| Imported unchanged portable-SIMD sources | `fixtures/upstream/rustc-checkout/library/portable-simd/crates/core_simd/src/**` | 35 originals via `tools/import_rustc_tests.py --support-file` (git blobs verified) |
| Repeated identical impl headers | `syntax/outline.rs` impl identity, `outline_associated_fragment`, `frontend/associated_declarations.rs` | `core::time` has two `impl Duration {}` blocks. The first keeps its header identity; later repetitions get an occurrence-qualified identity. |
| `use a::b::{self}` | `frontend/macro_names.rs` module routes, `frontend/names.rs` named imports and import targets, `frontend/module_aliases.rs` | A `self` leaf names the item `a::b` in the type namespace only. It never binds a value or macro (e.g. `crate::fmt::{self}`, `crate::cmp::Ordering::{self}`). |
| Predefined tool attributes | `syntax/outline.rs` classification; compiler-attribute preparation | rustc's `PREDEFINED_TOOLS` (`clippy`, `rustfmt`, `diagnostic`, `miri`, `rust_analyzer`, from `rustc_resolve/src/macros.rs:123`) are inert and do not disqualify `#[stable]`/`#[unstable]` items. |
| `final fn` in traits | `syntax/parse.rs::is_final_method_modifier`, outline parse-boundary check | The pinned parser predates rustc's `final` associated-function modifier (used in `core`). It recovers it as `ERROR(final)` before a correct `FN`; only that exact shape is accepted. |
| Dependency edges of unselected consumers | `SourceDiscovery::new_domain` | Like rustc compiling a dependency, a discovery ignores edges whose consumer crate it does not select. This is the shared single `SourceDependencies` row. |
| Optional Type-receipt facet | `type_scope_receipts.rs` | When the DB also holds a consumer's environment, the facet is unavailable instead of an error. |
| Module-candidate absence from directory listings | `cli/macro_export_modules.rs::recorded_candidate` | A recorded directory listing proves a candidate is absent, so full `core` prepares in one round instead of eight. |

## Limits removed (E-R5-006)

- About 760 byte and count ceilings, mostly a mechanical removal of `.filter(|n| *n <= CONST)` across 196 files. Kept for review: `256`/`depth` guards, protocol frame sizes, `jit/`, `native/`.
- Source-count caps on configured macro-export graphs, admission ceilings, the import-convergence work cap, and sealed-transfer ceilings.
- Tests that only asserted removed limits were deleted. Mixed tests keep their semantic assertions.
- Guest-execution accounting (fuel, time, memory, handles, frames) is in progress by a runtime agent under `vm/`, `runtime/`, `engine/` and `ir/`, with inert compatibility shims at outside call sites to be removed afterwards.

## Component status (preparation evidence, not runtime acceptance)

- **Cross-crate SDK control** `unchanged_cross_crate_call_borrows_sealed_aux_definition_preserving_actual_editions`: **passes**. The caller's `foreign_crate_macro_pat::custom_matches!` resolves through the sealed auxiliary owner and expands with its 2018 definition-edition and 2021 caller-edition tokens.
- **Full `core` preparation**: past portable-SIMD authority, duplicate impl scope, `self` leaves, tool attributes, parse of `final`, and several invented caps. The latest first failure is `use crate::cmp::Ordering::{self, …}` in `cmp::impls`, which is fixed in source but not yet re-run. Each run takes about 4.5 minutes in one round.

## Upstream runtime inventory (CLI, current tree, October 17)

Raw data is in `artifacts/runs/inventory/` (local; `summary.json`, `by-category.txt`). The 339 imported cases expand to 351 rows once revisions are counted.

- **Runtime contract:** 15 of 172 run-pass/run-fail configurations pass (14 run-pass, all also full-fixture; 1 run-fail, adapted). Four of the passes are an empty `main`.
- **Largest blockers:**
  - 101 rows: standard-library or builtin macros (`assert*!`, `panic!`, `println!`, `vec!`, `line!`, `format!`, …). In `main` these have no real expansion. In helper functions they currently run through the legacy runtime-macro route (audit MA007), which must be replaced rather than extended.
  - 31 rows: auxiliary crates are not loaded.
  - 12 rows: `use std::…` imports are unresolved.
  - 8 rows: `--test` harness discovery.
- **Missing next layer:** tuple, array and enum constructors, `Option` patterns, `Box::new`/`String::from`, closures, `for` loops, `&mut` compound assignment, `dyn` calls, and unsuffixed integer fallback.

## Update — later on October 17

Inventory with the current tree (`artifacts/runs/inventory/raw-v3.json`, 354 rows): **51 runtime-contract passes** (from 15), 21 completed check/build cases, 167 faults, 1 timeout. Probe programs for each mechanism are in the session scratchpad; nothing is committed.

| Mechanism | Where | State |
| --- | --- | --- |
| Real library macros on demand | `frontend/library_macros.rs`, `expand/source_declarative.rs` | `assert!`, `panic!`, `todo!`, `unreachable!`, `unimplemented!`, `println!`, `eprintln!`, `matches!`, `assert_eq!` expand from unchanged core/alloc/std sources; builtins (`format_args`, `assert`, `line`, …) go through the adapter's rustc-faithful expanders. `derive`/`test` now name the library's own definitions (fixed an ambiguity with a synthetic catalog identity). |
| `$crate` to another crate | `syntax/thunks.rs`, `frontend/callables.rs` (`$crate@<unit>` root) | Value paths emitted by library macros resolve in the defining crate through the ordinary graph. |
| Closed facade functions | `facade/functions.rs`, `engine/run_facade.rs` | `core::panicking::{panic, panic_fmt, panic_display, panic_str_2015, unreachable_display}`, `std::rt::{begin_panic, panic_display, panic_fmt}`, `std::io::{_print, _eprint}`, `alloc::__export::must_use`; selected by identity after ordinary lookup, completed in place at the call boundary (guest panic or resumed caller). `alloc::fmt::format` waits for `String`. |
| `format_args` | `syntax/format_args_thunk.rs`, `ir/format.rs`, `Op::FormatArguments` | Ordinary lowering of `builtin # format_args`: arguments evaluate once in order; the template (rustc grammar incl. width/fill/align/sign/#/0/precision/`n$`/`.*`, Display/Debug/hex/octal/binary/exp) renders scalars and `str`. Implicit captures and user `Display` impls are explicit gaps. Guest stdout/stderr appear as `output` in the run report. |
| Inert attributes | `syntax/original_inert.rs` | Root builtin-inert/tool attributes no longer demand a producer row. |
| Closed nominal/tuple/array signatures | `frontend/call_contracts.rs`, `frontend/source_parameter_goals.rs`, `frontend/source_types.rs::resolve_written_type`, `vm/context.rs`, `vm/frame.rs`, `runtime/cell.rs::{descriptor_source_term, intern_structural_term}` | Struct/tuple/array parameters and results. |
| Nominal drop glue | `frontend/member_index.rs::drop_types`, `GuestStore::install_drop_inventory` | Types without a `Drop` impl drop field-wise; user `Drop` impls still demand a witness (explicit gap). |
| Impl members | `frontend/member_index.rs`, `CallResolver::associated_function`, `source_types::impl_owned_name` | `Type::f(..)`, `Self::f(..)`, `Self` in impl signatures; names in impl functions resolve in their module. Generic impls are an explicit gap. |
| Method calls | `syntax/method_call_thunk.rs`, `engine/run_methods.rs`, `Op::MethodReceiver` | Receiver evaluated as a place; method selected from the receiver's reached nominal type (inherent, then trait impls); autoderef + `&self`/`&mut self`/`self` adjustment; then the ordinary call boundary. Primitive/str/slice methods need facade methods. |

Agents (same tree): pattern engine (one transactional engine for all owners; now extending to nominal patterns), nominal values (unit/tuple structs, enum variants, `Op::ValuePath`), numeric literal inference (component solver with i32/f64 fallback; now module-level signatures and compound assignment), dependency crates (`--extern`, aux-build; done), std facade crates and prelude (in progress).

## Open decision for the user (resolved: on-demand library macros)

Today, any standard-library macro (`assert_eq!`) requires sealing all of `core`/`std`: capture every module, expand every item macro to a fixed point, then resolve. A rustc-faithful alternative for an interpreter that assumes the program compiles is to resolve on demand, using original definitions, with expansion only when originals cannot answer. rustc rejects programs where a macro-expanded item would change an already-found answer. This changes the spec's discovery/seal model.

## Update — early October 17 (second lead session)

**Inventory scope.** All 2,465 remaining simple upstream run-pass UI tests (no aux/compile-flags/revisions/needs/only/ignore/edition directives) were imported unchanged with `tools/import_rustc_tests.py` (manifest now 2,807 cases, 3,476 files). Full run `artifacts/runs/inventory/raw-v6.json` (2,819 rows, 293 s wall with 8 jobs): **500 runtime-contract passes**, 25 completed check/build cases, 2,174 faults, 2 timeouts, 114 not-runnable compile-fail rows. The previous 354-row set went from 51–53 to 67 passes (`raw-v5.json`).

Largest v6 blockers: 426 `CATALOG-INCOMPLETE` (std imports/macros: needs the std facade on by default), 176 `reached direct function identity is absent` (std functions), 104 let annotations with reference/slice/`Box<_>` types, 80 unsuffixed numeric literals, 58 casts to non-primitive types (raw pointers have no runtime representation yet), 54 `_` argument hints, 50 unsuffixed literals inside tuples/arrays, 38 `unsafe {}` blocks (fixed after v6), 37 statics as places, 28 proc-macro auxiliaries.

**Preparation performance (failure mode 6 evidence).** Profiling (see `tools/profiling/README.md`; `sample`/`lldb` cannot attach here) showed dev runs dominated by unoptimized blake3 span keys and repeated tokenization. Fixes: `[profile.dev.package.*] opt-level = 3` for blake3/mbe/parser/syntax/tt/span/rowan/serde_json and related pure deps; span/context keys computed once in token-wire validate/sort/decode; `SourceDeclarative::owned_forest()` memoized per provider (`OwnedForestHandle`); parsed declarative definitions shared (`DeclarativeMacro::shared`); completed source assemblies built lazily per entered source (`LazySourceAssemblySeed`). Ten `assert_eq!`: 19.9 s → 1.4 s; `byte-literals.rs`: 51 s → 3.7 s; facade hello world: 11.8 s → 3.5 s. `LRR_PROFILE=1` prints stage/query exclusive times.

**Semantics fixed this session.** Body environments for module/impl callees selected by ordinary resolution (published before revision-bound contracts); `--test` catalogue discovery (contained Type facets only under scope seals); macro 2.0 items in assembled sources (generated-dialect refusal stays at definition capture); tuple/index projections auto-deref references (with or without a retained source view); borrows of closed non-primitive places; `unsafe` blocks; 5 whole-file `descendants().find` node searches in `syntax/thunks.rs` replaced with the O(depth) `syntax_at` (failure mode 8).

Agents active on this tree: nominal values/annotations, numeric literal inference, pattern engine/closures/consts, std facade crates, user `Drop`, removal of non-rustc limits (E-R5-006), procedural-macro auxiliaries.

### Later the same morning: agent round 2 and lead work

Full inventory `artifacts/runs/inventory/raw-v7.json` (2,819 rows): **561 runtime-contract passes** (+62 vs v6, −1 timeout under load), 38 executed check/build cases. Agent reports (each self-reviewed against the 8 failure modes; nothing committed):
- **Closures, fn items, constants** (pattern agent): closures are ordinary indexed syntax in their creating body (`ScopeKind::Closure`, `Op::MakeClosure`, `TypeKey::Callable`), calls reuse `PreparedSourceCall`/`enter_callee`; fn-item values and value calls (`syntax/value_call_thunk.rs`, `engine/run_value_calls.rs`); constants through `ValuePath` → `constant_item_value`. Remaining duplicate: `SourceConstant` vs `ValuePath` for constants (assigned to the scopes agent).
- **Numeric typing** (numeric agent): `assert_eq!`/`assert_ne!` type through the ordinary expansion; Value goals answer nested pattern bindings, qualified paths, facade calls, tuple/array/index facts.
- **Nominal values** (nominal agent): generic nominal construction, Copy nominal types, one `WrittenType` let-annotation route (refs, `str`, slices, `_`, `dyn`).
- **User `Drop`** (drop agent): `<T as Drop>::drop(&mut v)` as an ordinary guest call through a dispatcher continuation (`engine/run_drop_witness.rs`), rustc drop order incl. fields, temporaries, assignment, break/continue and panics.
- **Limits** (E-R5-006 sweep): ~1,050 non-rustc caps removed; kept const-eval frames (128), macro depth (256 — should become `recursion_limit`, default 128), type nesting depth.
- **Proc-macro auxiliaries**: compiled by `lrr run` preparation and executed by the stdio-v1 server (erratum **E-R5-007**, `docs/errata/revision5-standalone-proc-macro-preparation.md`); 12 of 18 runnable proc-macro cases complete.

Lead changes: associated-type projections `T::V`/`Self::V` (`frontend/associated_types.rs`, member-index associated types, normalization at argument hints/returns); blanket-impl method selection (`MemberIndex::blanket_functions`/`implements`); macro-generated impls in the single shared member index (`MemberIndex::add_generated`, `CompletedSourceEnvironment::admit_generated_units`; generated associated functions, `&self` methods via `generated_impl_self_type`, generated `impl Drop`); run-global static storage (`RunMachine::static_cell`, `ItemValue::Static`, place-context paths, `static mut` initializers, static assignment/compound assignment); `unsafe` blocks; clean refusal for operators on non-primitive operands (operator trait dispatch assigned to an agent).

Open next: operator trait dispatch; macros inside generated bodies; slices/ranges/`for`; body-local items and module value seals; aggregate constants; facade on by default; raw pointers; `--test` zero-test exit policy; unit-suite triage (1,245 pass / 184 fail before triage).

### Lead change review against the 8 recurring failure modes (source fingerprint: working tree, October 17 ~06:00; unchecked by a second reviewer)

| Mode | Lead changes this session | Finding |
| --- | --- | --- |
| 1 separate evaluator | `[e; n]` count is ordinary lowered syntax (usize-hinted) + one `Op::ArrayRepeat`; statics read/write through ordinary places rooted at run-global cells; associated-type normalization is a type service, not execution | none |
| 2 literal/break-only macro output | generated impls/methods/`Drop` join the one member index and `select_generated`; no output-shape special cases | none; macros *inside* generated bodies still unsupported (agent assigned) |
| 3 context-specific methods/inference | blanket-impl and generated-impl selection extend `select_method_call`/`associated_function` for every context; literal fallback (`bind_literal_fallback`, repeat elements) applies rustc's rule where inference left a literal unconstrained, not per macro | fallback should yield to the constraint service when it can type the literal (numeric round 3) |
| 4 generated-source rejection | removed outline-time macro 2.0 refusal for assembled sources; removed generated owner refusal in `select_generated` | refusal kept only at macro2 definition capture (pre-existing contract) |
| 5 duplicated patterns / eager arms | wildcard `let _ = place` uses no pattern machinery and evaluates nothing for locals | none |
| 6 repeated parsing/planning | `LazySourceAssemblySeed`, memoized `owned_forest`, shared `DeclarativeMacro` parses, one member index per completed environment (run no longer rebuilds it), generated unit outlines cached on the unit | `generated_impl_self_type`, `resolve_written_type`, member-index impl headers and `pure_constants::array_length` still re-parse token text/whole files — cold paths, tracked |
| 7 full-world scans | `Database::generated_syntax_units` iterates all inputs, only when a run has no completed environment (test harnesses) | acceptable cold path; production uses the environment's admitted units |
| 8 deep copies / whole-file node searches | five `descendants().find` lowering searches replaced by `syntax_at` | remaining whole-file walks: `pure_constants` planning and array-length lookup (tracked) |

## Update — midday October 17 (third lead session)

Nothing in this section is committed: the checkpoint commit attempted earlier failed because the SSH signing agent refused (`agent refused operation`), and signing was not bypassed. Pushed branch state is still `edd99619e3`.

**Harness fix.** `artifacts/runs/inventory/run_inventory.py` required the capture packet's guest streams to be empty (`closed_no_external_effects_observed`). Guests that print now close with `closed_managed_output_observed`; the check accepts that status and adds `guest_output_matches_plain` (captured guest stdout/stderr bytes equal the plain run's `output`). Under the old check, ~70 correct printing runs were filed as `completed_incomplete_evidence` (raw-v8: 559 pass + 78 incomplete). Rows recorded before this fix are not directly comparable: count pass + incomplete when comparing.

**Lead changes (working tree):**
- Impls on type aliases: `MemberIndex::add_outline` follows module aliases for impl self types; `frontend/source_types.rs` `deferred_alias_type` resolves a non-generic alias with a compound target (`type Foo = GenericFoo<u32>`) and generic aliases at use sites by walking the target in the alias's own declaration context (substituting written arguments). `lint/dead-code/with-impl.rs` now reaches receiver literal inference (numeric agent).
- Hot-path currentness (failure modes 6/7): `PreparedInputGuard` and `NamespaceWitness` memoize their last successful check per `(Database instance, input revision)` (`db::memo::CheckedRevision`); every input write increases the revision (verified across all `self.inputs` writers). `Database::lexical_namespace_stamps` is computed once per revision and shared as `Arc` (equal stamps compare by pointer first). `NameNamespaces::require_view` uses a per-publication memoized source proof; `PreparedSourceView::outline_item` is an indexed identity lookup used by `CallResolver::item` and the alias normalizer. A 2,000-iteration method-call loop: 24.9 G → 16.4 G instructions (debug), linear in iterations (~7 M/iteration, remaining cost: per-call contract preparation, reference-parameter proofs re-derived per call, field-type rebinding, arena allocation per value write).
- `Self::f()` regression: `CallResolver::written_trait` treats a `Self` root as "not a trait" instead of failing identifier validation.
- `#[loop_match]`/`#[const_continue]`: new `CompilerAttributeRequirement::ExecutionInert` (adapter `builtin_attributes.rs`), citing rustc_mir_build's lowering (same observable control flow). `loop-match/break-to-block.rs` and `loop-match.rs` complete.
- `&'static T` / `&str` results: structural result route (`vm/context.rs`), `GuestStore::{intern,find}_structural_term` handle `str` and references to structural targets; a reference result with no relatable closed reference input (e.g. `&self` of generic `Self`) falls back to the declared-type check (`closed_reference_results.rs`; no borrow checking, per scope). Gains: `cast/generic-trait-object-call.rs`, `inference/auto-instantiate.rs`, `resolve/struct-function-same-name-2445.rs`; four `associated-types-in-*` cases now stop on numeric literal inference.
- Removed temporary debug env hooks (`LRR_DEBUG_FOR`, `LRR_DEBUG_SCOPES`, `LRR_NO_IMPL_ENV`, `LRR_TRACE_BLOCK_BT`, `LRR_DEBUG_CALLS`, `LRR_DEBUG_VALUE_REPR`, `LRR_DEBUG_NAMES`; the slices agent removed `LRR_SLICES_DEBUG`). Still present, owned elsewhere: `LRR_TMP_NODEFER`, `LRR_TMP_NOSCAN`, `LRR_DEBUG_ENV`.

**Agent reports integrated this session** (each self-reviewed against the 8 failure modes):
- Architecture review (source reading only): top findings were per-execution `CallResolver::new` hashing (fixed above), static by-value reads re-resolved per execution (fixed by the scopes agent: per-PC `static_reads`), deep `SourceCallable` clones and a third return-admission route in operator dispatch (routed to the operators agent), four lang-item lookup mechanisms (one `LangItems` table recommended), duplicated pattern-target resolution between the constraint service and the runtime, error swallowing in the constraint service, eager retained-body admission with `let _ =` (held; see genbody report).
- Nominal round 3: compound field Types (`WrittenType::Parameter`), partial hints (`vm/frame_partial_hints.rs`), nominal members in tuple source views, qualified generic annotations, declared nominal results as context (`HintRecipe::DefinitionWrittenResult`, tag 10).
- Scopes: nested empty expansions in constants, casts/blocks/statics/associated array lengths in the pure evaluator, block-item patterns and local fn values through `EnteredValue`, E1 static read cache, prelude `None` special case deleted. 47 cases now report the const-evaluation **design hold** (nominal/reference/method-call constant initializers).
- Generated bodies: macros in impl/trait item position join the original impl/trait in the shared member index (`GeneratedAssociatedMember`); generated consts and generated `main` held.
- Type occurrences (triage agent): explicit type arguments and cast targets through the let-annotation written-type selection; `<Ty as Trait>::Name` returns; "entered Type actual indexed occurrence absent" 61 → 0.
- Slices round 2: implicit `&mut` reborrow on reads into reference-expecting slots (`vm/frame_coercions.rs`), `{:?}` of arrays/slices (`ir/format.rs` `List`).

**Open decision for the user.** Constant initializers of nominal/reference/method-call form (47 upstream cases, all `consts/const-enum-*`, `const-int-*-rpass`): evaluate through ordinary VM initializer frames (one pipeline; departs from spec §18.3's pure evaluator) versus extending the pure evaluator (a second evaluator, failure mode 1). Held until the user decides.

**Lib suite:** 1,367 passed / 61 failed (lead run, ~09:00). `engine::run::namespace_tests::{genuine_same_epoch_namespace_addition_…, actual_nonzero_cleanup_resume_…}` fail with or without the new per-revision stamp cache (old stamps change on a same-epoch seal publication), so they are not caused by it; root cause not yet traced.

# E-R5-010 — Function parameters with non-identifier patterns

October 17, 2026. Implementation record (not a spec contradiction) under the simple-interpreter scope (E-R5-006 looser than rustc, no resource accounting; E-R5-005 one pipeline). Extends the closure parameter-pattern work (`ClosurePlan`, `closure_pattern_parameter`) to `fn` items, impl/trait methods and block-local `fn`s.

## Violated invariant and minimal witnesses

Invariant: `fn f(PAT: T) { body }` behaves exactly as `fn f(arg0: T) { let PAT = arg0; body }` (rustc HIR lowering): every irrefutable parameter pattern runs the one shared pattern transaction, its bindings are visible to the whole body, are dropped after the body's locals in reverse declaration order, and by-reference bindings borrow from the argument value that lives for the whole call. Witnesses (rustc 1.99.0, all editions; today every one stops with `LRR-E-NAME-MISSING ... (value path `a`)`):

1. `fn f((a, b): (i32, i32)) -> i32 { a + b } fn main() { println!("{}", f((1, 2))); }`
2. `fn f(&x: &i32)`, `fn f(P { x, y }: P)`, `fn f(W(a, b): W)`, `fn f([a, b]: [i32; 2])`, `fn f(ref x: i32)`, `fn f((ref mut a, mut b): (i32, i32))`, `fn f((a, _): (D, D))`.
3. The same in an `impl` method after `self`, a generic fn, a block-local `fn`, and a fn emitted by a user `macro_rules!` (hygiene: a macro-introduced pattern name is not the caller's).
4. Drop order: `fn f((a, b): (D, D), c: D)` prints the drops rustc prints.

## Findings (read from source)

- A function's `BodyHandle.range` is exactly its block, so `index_body` never sees parameter patterns; closure parameters are inside the closure expression and are indexed. `parameter_from` (`syntax/thunks.rs`) maps only plain `IdentPat` parameters (read directly with `Op::ReadParameter` at each use), and `local_binding_at` stops at the enclosing `Fn`.
- Everything below the handle uses `body.range.start` as the origin of node-relative ranges (about fifty sites), so moving the handle's start is consistent for all of them; only the few sites that independently derive "the block range of an `fn`" must use the same helper (listed below).

## Design (one pattern engine, ordinary BodyIndex/thunk route)

A function whose parameter list contains a pattern parameter (`syntax/fn_parameters.rs::pattern_parameter`: anything but `x`, `mut x`, `_`; `ref x` and `x @ ..` count) is a *pattern function*. A function with only `x`/`mut x`/`_`/receiver parameters is byte-for-byte unchanged (same handle, index, recipes).

1. **Body extent** (`syntax/fn_parameters.rs`, `BodyHandle.block_offset`): the handle of a pattern function spans `[param_list.start, block.end]`; its content identity covers that span (UTF-8 bytes for text sources, the retained token/edition hash of parameter list and block for retained syntax: `ParsedFile::fn_body_identity`). `entered_body_root` finds the block from the `Fn`. `BodyHandle::block_range()` is the block alone, used where a declaration header excludes its body (`local_type_names`, `local_declaration_inputs`, `signatures`, `declaration_return_types`, `producer_closure`). The three handle producers (`outline.rs`, `body_items.rs`) and the two syntax-side derivations of "the body range of a function" (`macro_call_function_body`, the adapter's `entered_body`/`call_owner_body`, `declaration_scan/trait_headers.rs`) use the one rule. Editing a parameter pattern therefore changes the body identity.
2. **Index** (`index_body_with_cfg`): a `ScopeKind::Parameters` root scope (`ScopeId(0)`, the existing root-scope convention that already owns plain-parameter cleanup) encloses the block's scope; after the block's preorder the traversal indexes the `ParamList` subtree in that scope. The `ParamList` node is the pattern owner (`pattern_statement`, reserved Bool test slot, one staging slot per written parameter), exactly the reservations a closure node makes. `BodyIndex::parameter_owner()` answers whether a body is one. The block stays `NodeId(0)`, so every consumer of "the function entry" is unchanged.
3. **Execution**: the root block's `Entry` additionally enters the parameters scope before its own scope and continues into the owner's chain instead of the first statement (`block_first_pc` is shared). `ClosurePlan` is the shared call-frame parameter plan: the function variant binds *every* written parameter in order, as `let PAT = arg;` would: a pattern parameter through `Open`/`Test`/`Commit`/`Close`, `x`/`mut x`/`_` through `ReadParameter` + `BindLocal` (the closure's own binding variant). Binding all parameters uniformly gives rustc's reverse-declaration drop order across plain and pattern parameters; the receiver stays frame parameter 0.
4. **Names**: `local_binding_at` resolves a name against the enclosing pattern function's parameters (through `pattern_binding_at`/`index.pattern_bindings()`, the same hygienic group lookup `let`/`match`/closures use) after any inner binding; `parameter_from` returns no frame parameter for a pattern function's written parameters. Parameter binding recipes become visible from the body block (`visible_from`), and `pattern_binding_occurrences::owning_pattern` joins parameter patterns to their `BindingForm::Parameter` recipes, so the ordinary Value goal selects them.
5. **Static constraints** (`syntax/source_constraints.rs` fact `parameter`, `frontend/binding_constraints.rs`, `binding_constraint_modes.rs`): the parameter root patterns are pattern roots whose matched Type is the declared Type of that signature parameter (`parameter_term`), exactly as `let PAT: T = ..`.
6. **Cleanup**: `ScopeKind::Parameters` is "bindings first" like `Clause`: the staged remainder of a destructured argument outlives the bindings taken from it, and the whole scope is left after the body's locals.

## Shared entry points extended / alternate routes

Extended: `ParsedFile::entered_body_root`, `index_body_with_cfg`, `initial_scope_thunk` (root block Entry), `PatternSourcePlan::owner_for`/`prepared_pattern`/`lower_closure` (generalized `ClosurePlan`), `pattern_owner_test_slot` and the `Open` source check in `ir/validate.rs`, `local_binding_at`/`parameter_from`, `Frame::bindings_first`, `SourceConstraintPlan::prepare`, `Component::root_matched_type`/`pattern`/`declared`, `PreparedPatternBindingOccurrences::capture`, `role_matches` (node lookup of the `ParamList` owner), the outline/body-item handle constructors. Alternate routes added: none. Not added: a parameter-specific matcher, a rewritten `let` prologue in syntax, a per-call re-index, a macro- or context-specific rule.

## Cold/hot cost

Cold, once per pattern function body edition: the `ParamList` subtree in the index, one `PatternSourcePlan` memo (keyed like every pattern owner), pattern trees for the parameter roots in the constraint plan. Functions without pattern parameters pay nothing (one `runs_patterns` scan of the parameter list when a failed local lookup reaches the `Fn`). Hot, per call: the argument check, two `ScopeEnter`s and per parameter one `ReadParameter` + `BindLocal` or `Open`/`Test`/`Commit`/`Close` — what `let PAT = arg;` runs; no source preparation at call time.

## Evidence

- `cli/fn_param_pattern_tests.rs` (10 tests, editions 2015-2024, outputs from `rustc +1.99.0`; the generator checks all four editions and the macro/wrapper forms agree): pattern families handwritten / user macro / wrapper macro; methods after `self`, trait default, generics, block-local fn; unsuffixed literals typed from the parameter; closures, shadowing, loops, `return`, recursion, exact-once argument evaluation; fn items as values and enum single-variant patterns; macro-emitted pattern parameters (hygiene, wrapper, impl-member macro); body macros; drop order (including plain beside pattern parameters, `_` parameters, `ref`, receiver); plain/`mut`/`_`/receiver controls.
- `syntax::fn_parameters::tests` (3): handle extent, block offset, index shape (`Parameters` scope parent of `Block`, owner reservations, staging slots, parameter nodes in the parameter scope), plain functions keep a block-only handle and no `Parameters` scope, identity changes with the pattern, `ref`/`@` make a pattern function.
- Unchanged upstream fixtures newly passing because of this change (A/B on the same binary: `binding/func-arg-incomplete-pattern`, `binding/func-arg-wild-pattern`, `array-slice-vec/destructure-array-1`, `or-patterns/let-pattern`).

## Known gaps

1. A by-value destructured parameter's `_` remainder (`fn f(x: D, (a, _): (D, D))`) drops after the earlier parameters' bindings (rustc drops it right after `a`, before `x`); all other mixes of plain, `_`, `ref` and pattern parameters match rustc.
2. A macro call in parameter pattern position (`fn f(deref!(ref x): Box<usize>)`, `binding/func-arg-ref-pattern`) stays the "blocked macro" refusal; a macro call inside a parameter's *type* is not collected by `entered_macro_facts` (its traversal starts at the block).
3. `#[cfg(..)]` on a parameter of a pattern function is not exercised: the plan binds every written parameter and does not skip a cfg-disabled one (the existing frame-parameter ordinal counting does not either).
4. `fn f([a, rest @ ..]: [T; N])` stops in the matcher (`array subslice binding requires its sized subarray value`, `array-slice-vec/fixed-length-vector-pattern-matching-7784`), as `let` does.
5. Closure patterns with typed literal arguments (`binding/fn-pattern-expected-type`: `|(x, y): (isize, isize)| ..` called with `(1, 2)`) stop in `LRR-E-OP-TYPE`: closure expected-type propagation, not a function parameter.
6. `const fn` with pattern parameters stays in the pure constant evaluator's domain (`consts/const_let_eq`).

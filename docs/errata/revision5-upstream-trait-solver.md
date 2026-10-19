# E-R5-014 — Reuse the next-generation rustc trait solver

October 19, 2026. User-directed architecture amendment following the discussion of upstream solver reuse. This replaces the custom-solving implementation choice in revision5 Sections52–53 and M020–M024. It is a requirements change, not evidence of an implemented integration or a performance comparison.

## Required reuse

Use `rustc_next_trait_solver` with `rustc_type_ir` from the existing upstream Rust/test pin `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`. The inspected checkout contains the generic `Interner` and `SolverDelegate` interfaces. The [compiler development guide](https://rustc-dev-guide.rust-lang.org/solve/sharing-crates-with-rust-analyzer.html) documents their reuse by rust-analyzer. This establishes a concrete integration route, not a claim that LRR's adapter is already available.

Do not implement a separate trait solver, candidate-transaction engine, canonicalizer, projection solver or cycle/SCC scheduler where those semantics are supplied by these shared crates. Reuse their inference machinery through the required adapter interfaces. Do not replace the new solver with rustc's legacy solver, Chalk or a fixture-specific subset. Mechanical build adaptations must be explicit, provenance-pinned and behavior-preserving. Any source-equivalent published crates require exact provenance verification. No unsupported assertion that upstream is too slow justifies a fallback.

LRR retains its independent parser/source graph, custom query database, ordinary lowering/VM, source-aware member lookup, runtime representations, dictionaries and eventual JIT. Implement the bridge to actual trait/impl/member identities, type terms, binders, ParamEnv, language items and relevant complete source/header catalogs. Keep these responsibilities distinct from the upstream solving algorithms. A constraint response must be connected to real selected-impl/member evidence before execution; a matching runtime tag is insufficient.

## Demand and execution boundary

Ask only goals needed by reached operations, argument/result inference, associated projections, layout or cleanup. Fulfill their necessary transitive header predicates, preserving symbolic definition context and all official editions. An unused ordinary function bound stays unforced. No whole-body rust-analyzer/rustc inference, borrow checking, static privacy/lint/coherence enforcement or implicit cargo check is introduced. Do not compile or type-check a selected live runtime unit with rustc to answer a goal.

Original and generated code use the same adapter and execution pipeline. A generic definition's source-selected trait member must not become an actual-type inherent method. For example, a concrete type can have both an inherent method and a trait method of the same name; a direct call and a call through a generic trait bound retain their respective source meanings. Conditional impls, associated types and numeric conversion input/output constraints still require real resolution, even when demanded during runtime preparation.

The upstream callbacks consume admitted immutable facts. Missing source or expansion input suspends to the existing coordinator through NeedInput; callbacks do not perform IO, execute macros or run guest code. Preserve completed argument effects and VM continuations across re-entry. Bind final answers and any retained upstream cache to actual source/epoch/world dependencies, including negative/ambiguous candidates. Do not add source parsing or a full namespace comparison to installed execution.

Only pinned-rustc limits are permitted under E-R5-006. Record the actual compiler definitions/configuration for each enforced limit. Remove the old LRR table/strand/work/byte budgets; do not translate cancellation or incomplete source evidence into a semantic negative answer. Reached constant/static values continue through E-R5-007's ordinary VM initialization; upstream header/preparation const requirements retain the explicitly pure preparation contract.

## Migration and acceptance

1. M019 supplies actual source headers and dependency-aware candidate catalogs to the adapter.
2. M020 first establishes a macOS Rust1.99.0 build with exact shared-crate/dependency provenance, source/type/binder/ParamEnv translation and genuine upstream goal evaluation. A build or source-only bridge is component evidence.
3. M021–M024 integrate upstream conditional selection, search-graph/limit behavior, projections/GATs and higher-ranked constraints. Preserve the existing TR witness requirements; replace custom algorithm and reduced-budget tasks with upstream bridge/semantic parity checks.
4. M025–M027 connect source member identity, lazy dictionaries, ordinary calls and relevant invalidation. Compare source-selected behavior across original/user-macro/wrapper/standard-macro contexts and all editions.
5. Keep existing custom semantic paths explicitly held until their consumers migrate, then delete the superseded production routes. Historical uncompiled custom canonical/table candidates cannot be promoted into the new architecture. Preserve their evidence as history.

Use unchanged upstream rustc runtime fixtures and actual directives/auxiliaries as primary witnesses. Include failed-candidate rollback, conditional bounds, recursive projections/auto traits, higher-ranked constraints, associated outputs, numeric conversions, cold-body/unused-bound behavior, exact-once effects and relevant source invalidation. Prove the actual upstream route is used, not only matching final values. A rustc-only comparison is separately labeled evidence and never an interpreter pass.

The interpreter runtime prerequisite, separate diagnostic/tooling/full-fixture gates, macOS-first priority and guest-JIT ordering are unchanged. No task or gate passes because this document is written. The inherited simple structural unifier and source-only canonical/table proposals remain migration debt; integration and measurements must establish correctness before completion or speed claims.

## Source references

- [Pinned shared solver](https://github.com/rust-lang/rust/tree/ae0fe511b15ab760d7e963b13389d1bc2ab42b18/compiler/rustc_next_trait_solver)
- [Pinned SolverDelegate](https://github.com/rust-lang/rust/blob/ae0fe511b15ab760d7e963b13389d1bc2ab42b18/compiler/rustc_next_trait_solver/src/delegate.rs)
- [Pinned Interner](https://github.com/rust-lang/rust/blob/ae0fe511b15ab760d7e963b13389d1bc2ab42b18/compiler/rustc_type_ir/src/interner.rs)
- [Pinned search graph](https://github.com/rust-lang/rust/blob/ae0fe511b15ab760d7e963b13389d1bc2ab42b18/compiler/rustc_next_trait_solver/src/solve/search_graph.rs)

## Document validation

`python3.12 tools/spec_consistency_audit.py` and `python3.12 worktracks/check.py` pass. The existing document-audit regression suite (`python3.12 -m unittest discover -s tools -p test_spec_consistency_audit.py`) passes52tests. These are document/plan checks only. The pinned checkout revision and shared-crate interfaces were inspected; no shared-solver build, adapter execution, runtime acceptance or benchmark was performed for this amendment.

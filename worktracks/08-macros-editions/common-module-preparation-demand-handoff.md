# Exact module preparation demand — LOCAL-024

M013/M014, revision-5 Sections 50, 55 and 57; October 16, 2026.

The unchanged configured core root imports `prelude::rust_2024::*` at
`library/core/src/lib.rs:201`. Diagnostic204 compiled successfully but its
required full-core positive failed before discovery polling. The exact missing
resolved module path is `prelude::rust_2024`; the external declaration is
`pub mod prelude;`, and `rust_2024` is inline in its actual child source.

Invariant: an incomplete source graph must request actual preparation, never
publish macro absence or a graph with a silently dropped glob. Extend the
existing `frontend::module_aliases::graph_with_compiler_imports` worklist with
a typed preparation outcome. Carry the actual `SourceImport` and its resolved
missing module path. The existing strict entry points convert this outcome
back to their existing diagnostic. There is no permissive graph, synthetic
producer, alternative path resolver, source read, capture or authority grant.

`SourceMacroRoutes` exposes this same kernel to the pre-route preparation
owner. The coordinator must join the demand to an actual retained
`SourceModuleDeclaration`, perform existing `CaptureModule` capture/adoption,
and rebuild the coherent configured source/facet/adoption publication before
installing compiler routes. A missing path alone is not a module declaration
or capture authority. SourceDiscovery original-module support is peer-owned;
the source-publication/coordinator join remains an explicit integration hold.

Costs: the existing cold bounded module worklist is unchanged. At the failing
glob only, retain one bounded import and resolved path; no installed execution,
dispatch, parser/body clone, global invalidation or hot source walk changes.
Do not retry the same graph without a changed actual source dependency.

Architecture review scope: recurring modes 1–5 add no evaluator, macro rules,
numeric/method selection or patterns. Modes 6–8 retain the existing cold
worklist and add no execution-time planning, world census or BodyIndex copies.
MA001–MA008 remain open. Review must be tied to the frozen source fingerprint
before validation; source-only changes do not close this repair.

Closure evidence requires the unchanged full-core positive to capture its
required modules through the actual preparation path, retain their candidate
and role dependencies, then seal and expand normally. Further failures remain
failures. A typed demand control alone is component evidence, not that result.

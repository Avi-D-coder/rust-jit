# M013 generated external-module source frontiers

M013 remains in progress. Generated header planning previously ignored external
module-source events that were not ordinary macro/import blockers. A generated
`mod child;` could therefore be treated as an empty completed module. Planning now
extracts active external-module requests from the existing pinned owned syntax tree,
without reparsing output text or reading files, and reserves a source frontier for
every active child declaration. cfg-disabled declarations contribute no demand.

Each typed demand retains its generated header key, logical child scope, module
path/directory/path override and actual owned token-span origin. A deterministic
bounded module-header index rejects ambiguous attachment. Demand identities bind
the structural header and child scope, including the source-origin identity. The
structural path survives an unrelated function-body edit; changed captured origins
invalidate the demand conservatively. Source discovery retains admitted unresolved
demands in a bounded ordered index. Neither this metadata nor exposing it grants
file access, discharges a frontier or permits seal publication.

Six controls verify namespace/CLI incompleteness, cfg exclusion, logical path and
origin preservation, structural-path stability with origin invalidation, and retained
unresolved demands without capture or sealing. The first test-harness attempts used
a private API and did not compile; those logs are not passing or functional-baseline
evidence. The initially overstrict identity assertion was corrected to require both
stable structural paths and source-origin invalidation; no production origin binding
was removed to make it pass.

The real native macOS ARM64 Rust1.99.0 probe executes a macro-emitted child module.
LRR currently refuses that same positive case with an unresolved source frontier.
The native witness explicitly records status=pending and comparison_passed=false.
It is not a passing compatibility fixture or completed module-capture requirement.

The next integration must explicitly capture and adopt module candidate bytes and
absence/membership facts under source/build-output authority, reject ambiguous or
escaping paths, enter the fresh child scope with its retained parent macro context,
schedule child declarations/imports/macros through the same fixed point, and discharge
the exact frontier only after completion. No filesystem probe belongs in a query.
Actual child-function execution and native comparison are mandatory before claiming
generated external-module support.

Full compiler/SDK attribute inventory/import/dispatch, procedural strings, cross-unit/
world/edition and Cargo/facet preparation, complete origins/invalidation/quotas,
remaining compiler-expression and type/runtime integration, M014–M016, full
MX027/MX028/JIT and release acceptance remain required. Linux is deferred.

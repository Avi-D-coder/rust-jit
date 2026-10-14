# M007 original source module glob contributions

Module glob imports now retain weak module candidates separately from declared
modules and named aliases. Candidates carry canonical defining-unit/module
identity and the intersection of source-member and import visibility. Explicit
bindings suppress weak candidates; diamonds deduplicate identity while distinct
accessible targets return sorted typed ambiguity. Private members inaccessible
at the import origin do not become disclosed glob members.

The source builder recomputes named alias closure and weak glob contributions
until the candidate state settles. This supports forward/chained module globs
and named aliases of their members. Rooted glob cycles propagate disclosed
members; empty cycles settle empty. Explicitly encoded repeated state detection
rejects unstable strong/glob cycles. Source/import/header/metadata/depth/work
limits remain checked. Results belong to the immutable candidate graph, without
executing queries, build tools or macros.

Direct module and qualified macro lookup share the registry. Canonical glob
members retain checked import visibility; explicit aliases retain original target
paths and check those paths from their import origins before returning to the
caller. Three source checks cover forwarding, explicit shadowing, ambiguity,
visibility, diamonds, rooted/empty cycles and renamed dependency/facade unit
identity. A native macOS Rust1.99 fixture executes five permitted macro paths
through module globs and requires ambiguous_glob_imports and E0603 for rejected
paths. It compares disclosed module and macro identities; it does not exercise
the guest VM.

Ordinary-item glob facets, producer/generated-source completion, membership
seals, relative invocation AST integration, local bindings and guest VM dispatch
remain pending. M007 remains in progress. Aggregate expansion arena/deadline
accounting is still M012; no full performance/security/release claim is made.

# M013 extern-crate declaration identity across generated fragments

M013 remains in progress. An explicit extern alias now retains a typed, stably encoded
declaration identity as well as its lexical scope, name, recorded provider and
visibility. Generated aliases replace their temporary source DefKey with the exact
GeneratedItemKey. The bounded module worklist retains declaration identities in a
separate explicit index: repeated views of one declaration are idempotent, while
distinct declarations at the same lexical path are refused even for one provider.
Named module imports cannot merge with an explicit extern declaration. A macro import
can share the spelling because it belongs to a different namespace.

The pinned adapter has a distinct extern-crate header kind, appended as stable tag13.
Expansion output containing only extern declarations now retains a macro catalogue;
it previously bypassed namespace contribution capture and could silently seal duplicate
aliases. Generated extern headers no longer declare a fictitious local source module.
Validated captured aliases install in the ordinary namespace module-prefix tables and
frontier index, preserving actual provider membership and visibility. This discharges
macro imports through the alias after their real source producer completes. A matched
recorded extern-prelude edge can receive its explicit declaration's visibility; local
module declarations and unrelated aliases cannot be replaced. Owned alias metadata,
paths and registry updates have checked limits.

Twelve new controls cover original/generated, sibling, repeated and nested conflicts;
independent module scopes; generated macro imports; named module-import collisions in
both directions; separate macro/type namespaces; explicit public visibility on the
matched prelude name; and same-database edit invalidation. Conflict diagnostics poison
the discovery request: repeated polls preserve the same refusal. The edit invalidates
the old ready seal and refuses stale publication without a database revision change,
then refuses the new conflicting declaration set.

Four actual VM executions match isolated native Rust1.99.0 macOS ARM64 binaries.
Six native refusal witnesses require E0259 or E0254. Native compilations are explicit
oracle preparation only. No LRR query executes build tools, no selected live runtime
unit is compiled by LRR, and no arbitrary native symbol is loaded. Initial failures,
partial repairs and the import trace are retained; temporary test tracing was removed.

Rust2015 explicit/prelude rules, anonymous extern declarations, extern-crate macro_use
and selective forms, complete origin overlap/invalidation and remaining generated
module capture, Cargo/world/facets, complete MX027/MX028 and JIT acceptance,
runtime/type/quota integration and M014–M016 remain mandatory. The full project,
release acceptance and the later Linux port remain unfinished.

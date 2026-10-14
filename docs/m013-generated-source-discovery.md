# M013 generated-fragment source-discovery integration

SourceDiscovery now admits root fragments through GeneratedMacroCatalog::for_discovery
and schedules a deterministic child-discovery job when the fragment contains active
item-position calls. Polling exposes a separate immutable NeedGeneratedExpansion
demand. Repeated polling shares it; execution remains an explicit caller action using
the pure pinned matcher, with no query-provider execution or tool authority.

A child reply must match the exact pending key. Leaf completion checks aggregate
record/wire/header budgets and the discovery update before publication. Generated
headers preserve the actual marked call context. The captured caller scope and root
context are carried separately from the parent fragment; the owned token context
ancestry must reach that root. Hygiene marks do not invent scope ownership. The public
root header-admission contract remains strict.

The parent job stays active until all generated calls complete. Its original textual
producer remains unresolved and generated namespace frontiers remain reserved.
Only then are outward bindings published, frontiers released and the job completed.
The final discovery certificate binds both parent and child immutable records.
Original imports are rerun against actual published headers. Unknown or qualified
calls remain incomplete. Generated outputs with further producers or imports retain
the exact pending demand and cannot certify missing names.

Four integration controls exercise real matcher output: a generated leaf function
satisfying an original import with both record inputs bound into the seal; generated
calls completing before outward original-source use; cfg filtering and repeated
ordered calls with unresolved preceding calls remaining incomplete; and recursive
unknown output refusing publication while retaining the exact demand.

M013 remains in progress. Full enclosing-source resolution, recursive generated
bindings/imports, qualified/contextual paths, definition-unit routing, immutable
query database publication, complete quota boundaries and publication-failure
injection remain required. MX027/MX028 runtime fixtures and full E24/release gates
have not passed. M014 follows in the fixed ledger order. Validation is macOS ARM64
with Rust 1.99.0; Linux acceptance is deferred and must be real before claiming support.

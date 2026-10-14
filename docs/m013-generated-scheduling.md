# M013 generated ready-call scheduling and binding origins

M013 remains in progress. In the unsealed source driver, a generated fragment now
chooses the first ready pure call in stable source order, revisiting blocked calls
after independent producers complete. The pending request remains exact and stable.
Completion uses its immutable canonical call position rather than assuming it was
the first queued call. Standalone generated lookup retains strict ordering, and all
original/generated selections still require full revalidation before sealing.

A native probe exposed a separate false success: an unrelated generated textual
macro was allowed to override an imported alias. The path graph now records explicit
binding origins for definitions, named edges and glob edges. Generated contributions
use their exact record origin; original/SDK contributions use their explicit root.
Owned metadata is bounded before cloning/admission, and visible edge counts are
validated before origin mutation. Lookup retains visibility and strong/glob priority
and ignores retired weak edges. The existing ancestry selector compares textual
and import binding origins for both generated and original calls. A descendant
import may remain shadowed; an unrelated origin conflict refuses publication.

Six controls execute the independent producer, retain a true cycle, refuse original
and generated import/textual ambiguity, accept a descendant import, and check origin
priority/retirement plus failed metadata admission without publishing an edge. Two
initial scheduling controls failed. The first milestone gate exposed an older
control that assumed a blocked first call hid its ready leaf; it now requires the
leaf to expand and the remaining unknown producer to stay incomplete. The earlier incorrect VM success and native
E0659 probe are preserved in the alias-before log/oracle. The native matrix compares
eight successful VM cases and five actual E0659 refusals. Isolated native copies
allow the macro-export path lint and never compile/load the selected live unit.
This is an implementation repair, not a pinned-backend contradiction.

An admitted blocked nested child still occupies its parent pending slot; scheduling
independent parent siblings across that nested suspension remains pending. Full
origin overlap/invalidation, Cargo/world/definition-unit facets, generated-body
runtime macro contexts, type/quota/failure boundaries and full MX027/MX028 fixture/
JIT matrices remain mandatory. Test/generated catalogue work follows fixed M014–M016
order. These are macOS ARM64 Rust 1.99.0 component controls; release and Linux
acceptance remain pending.

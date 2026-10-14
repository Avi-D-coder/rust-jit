# H001 graph parser witness

This immutable fixture copies the pinned Cargo planner's captured lib-tests graph
from artifacts/compile-time-probe/driver-96947-1791290330487833000/lib-tests-unit-graph.stdout.
It is used for pure schema/edition/identity checks, not to execute source paths or
claim new Cargo compilation. The original evidence retains planner/compiler logs.
Tests mutate only recorded edition fields and parse the resulting graph; no build
tool is invoked by this fixture test.

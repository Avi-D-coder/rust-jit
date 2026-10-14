# M013 generated inline declaration containers

M013 remains in progress. The pinned token-tree adapter now traverses inline
module declaration lists, retaining each header's structural path and original
span/context. It uses bounded iterative traversal; it does not enter function,
closure, const or associated bodies. Generated item identities use the actual
container path rather than flattening nested headers into root ordinals.

Active nested ordinary headers and imports enter the existing namespace graph
with their actual module scope, canonical identity and visibility. Generated
module declarations register module membership so ordinary aliases/reexports
resolve through their original container. Nested generated syntax/body inputs
retain the same source scope. Caller and callable checks use the validated
per-declaration scope, so unqualified calls inside a generated module resolve its
own members. Cfg applies before active registration; cold ordinary body type
errors remain unlowered.

Nested import frontiers now belong only to their actual importing module. The
first integration run exposed a duplicate root reservation that the nested import
resolver could not retire; the second failed log preserves that witness. The
original module tests and intermediate caller-scope failures are also retained.
A separate regression proved that a completed job ledger could return a seal
while its requested namespace retained a frontier. The source driver now validates
the requested namespace certificate before returning an outward seal. Publication
continues to validate immutable inputs and refuses partial/stale certificates.
These are implementation repairs, not pinned-backend contradictions.

Eight controls exercise nested member calls, imports inside/outside a module, cfg,
module aliases, same-named sibling members, unresolved producer nonpublication,
visibility/structural identities and the retained-frontier seal refusal. The VM
examples run actual generated callable bodies. Isolated macOS Rust 1.99.0 oracles
add five successful module examples, bringing the matrix to seventeen successful
VM/native comparisons; seven earlier E0659 cases remain. Native copies never
compile/load the selected live runtime unit.

Generated module item-macro lexical scopes still require full integration; unknown
nested producers retain an explicit incomplete state. Complete Cargo/world/
definition-unit facets, origin invalidation, generated-body runtime macro contexts,
return/generic alias and quota/failure boundaries, complete MX027/MX028 fixture/JIT
acceptance and release remain mandatory. M014–M016 follow Section 57 order. These
are macOS ARM64 component controls; Linux remains a later port.

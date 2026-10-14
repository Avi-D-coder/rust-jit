# E-M007-003: complete reexport cycles are empty

The initial M007 namespace index returned `LRR-E-MACRO-SCOPE` for a cycle with
no originating macro definition. Revision 4 Section 50.2 requires a complete
empty result when all producers are known. The minimal witness is two canonical
paths `a -> b -> a`, with no pending producer and no definition. The before-fix
regression fails with `unresolved cyclic macro reexport` in
`artifacts/macos-first/m007-cycle-before-correction.log`.

Return `Absent` for that completed graph. A pending producer anywhere reachable
still yields `NeedInput`; no absence is memoized. Add atomic producer publication:
validate all definitions and resource sizes before removing the pending input.
An empty publication completes the frontier; a later originating definition
becomes visible through the same cycle. Stale or malformed publication leaves
the demand and all bindings unchanged.

The source index must register every unknown producer before lookup. This
component walk does not provide the source/catalog seals required by M010/M014.
Their fixed-point integration remains mandatory before runtime negative caching
or full MX011/MX012 acceptance. This corrects LRR's implementation to the existing
specification; it changes no pinned frontend, query framework or language rule.

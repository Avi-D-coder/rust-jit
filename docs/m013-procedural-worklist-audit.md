# M013 remaining integration: procedural source producers

Revision 4 Sections 50.2, 50.4, 50.5 and fixed Section 57 order.
Evidence inspection after the startup deadline implementation, macOS first.

SourceDiscoveryStep in crates/lrr/src/expand/source_discovery.rs currently exposes
SourceDeclarativeRequest, GeneratedCallRequest, Sealed and Incomplete. Its ready
producer paths do not expose an owned procedural expansion request. The standalone
CLI coordinator in crates/lrr/src/cli/compile_inputs.rs directly expands the two
existing request variants. MacroRun and MacroSession own separately authorized
matched-server execution and exact-key adoption, but this is not yet an integrated
procedural producer path in SourceDiscovery. Component session success therefore
cannot prove a procedural-generated namespace has reached the source fixed point.

The next M013 invariant is to suspend on a source-resolved procedural producer,
retain its exact unit/epoch/export/input/context identity and frontier, execute it
only through the authorized coordinator/session, then integrate its validated
record through the same worklist before any scope seal. The provider must remain
pure. Missing authority, wrong replies, failed execution and unresolved producers
must keep the scope incomplete and prevent guest entry. Sharing the existing
run-owned output budget must avoid executing or charging the same output twice
for another discovery consumer.

Acceptance must exercise an actual prepared proc tool and the matched server,
generated import/macro fixed-point progress, unchanged frontier on refusal,
exact-key/epoch rejection and scope publication only after successful integration.
It must not substitute a fabricated successful proc record or a native-only
comparison for this source-to-coordinator path. Full transformed test/impl scans
remain in the subsequent M014/M015 tasks; global release evidence remains false.

M013 remains in progress. This audit records inspected missing integration, not
acceptance, a new architecture, a smaller project goal or an ordering exception.

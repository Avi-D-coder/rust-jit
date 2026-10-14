# M013: ordinary module readiness and leaf completion

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 remains in progress. Full ordinary module publication and CLI execution,
macro contexts and the final discovery fixtures remain required.

Invariant: a parent source producer can retire only after its actual child parse,
imports, nested producers and expansion records are complete and adopted input
stamps are current. Readiness alone cannot retire a producer or publish a seal.

Generated and ordinary syntax now share readiness checks while retaining distinct
typed provenance. Ordinary completion snapshots the real child macro index into
its actual parent, installs final routes and releases only that child's frontier.
Plain modules do not export textual macros; macro-use modules retain the existing
explicit import path. Completed ordinary source owners are retained for subsequent
integration; body-owner dispatch and complete runtime contexts remain unfinished.

Four controls cover pending parsing, ready leaf completion, nested module and
missing import obligations, a substituted consuming environment request after a
positive readiness result, and exact completion replay. Replay still re-observes
the parent capture, adopted environment, child capture and all compiler stamps;
it neither changes the database revision nor publishes discovery. New capture
preparation remains restricted to pending producers. The replay failure before
the fix is preserved in its test log.

The historical nested-module CLI/native witness remains pending: LRR reports
catalogue-incomplete while the isolated native reference prints 2053. This component
change does not claim that witness passed. Full quota boundaries, broader source
combinations, service/facets/cache/runtime/type integration, MX027/MX028, production
JIT, release artifacts and Linux acceptance remain mandatory.

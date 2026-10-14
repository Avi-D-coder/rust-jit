# Historical bootstrap state — superseded

The platform and sequencing instructions below describe the earlier revision-3
bootstrap. Current instructions are in AGENTS.md and implementation-goal.md.

# Implementation state

The saved Codex goal is to complete the entire revision-3 specification, including
T000–T122, H001–H028, every required gate, and reproducible release evidence. The
goal remains active. The full runtime is not implemented or release ready.

Completed: T000–T003 bootstrap and H004 controlled Cargo reuse experiment.
Linux x86-64 Rust 1.99.0 gate 00 passes with 41 unit/integration tests plus one
compile-fail doc test. The parser and actual Cranelift functions execute; an
ambient libc import is rejected in a child process. Dependency pins, exact vendor
patch, license declarations, architecture boundaries, and fixture references are
checked. The task ledger contains every original and hybrid task card.

T010 stable keys and typed IDs and T011 immutable source capture are implemented
and covered by the available host regression gate. They remain in progress until
milestone 01's full acceptance coverage is integrated. Current behavior includes
strict digest JSON, stable explicit field encodings, edition-sensitive identities,
nonwrapping IDs, checked source paths, bounded capability-rooted reads, external
module resolution, metadata-based race retry, immutable source bytes/maps, approved
binary includes, and atomic in-memory overlays with module-graph validation.

The controlled race fixture changes file length on every capture attempt. An
initial equal-length rewrite relied on timestamp precision and did not reliably
trigger the specified metadata checks on Linux. Its failing gate report is retained
as artifacts/gates/00-before-race-fixture-fix.json. The corrected test guarantees
observable metadata changes without sleep or stronger filesystem-atomicity claims.

Cargo probe failures on the macOS bind mount and the final passing Linux-storage
experiment are retained and explained in cargo-reuse-probe.md. Do not erase them
or convert the failed runs into passing evidence.

Next: finish milestone 01 in the specified order, starting with T012's owned
syntax outlines and body/header fingerprint separation, then T013–T015's pure
query memoization, negative dependency publication, budgets and source remapping.
Apply H001's edition/profile identities to every new cache interface. Implement
gate 01 with all acceptance tests before advancing to the frontend/LRIR milestone.
H002/H003 belong to later control/pattern work and are not complete. Follow the
remaining original tasks before H005–H028. No alternative frontend or AST evaluator
has been introduced; missing guest execution currently fails explicitly.

Use the pinned Linux container for target validation. Use /tmp inside Linux for
the controlled native-workspace probe. The command logs and task ledger record
what actually ran. Do not use emulated timings for native performance claims.

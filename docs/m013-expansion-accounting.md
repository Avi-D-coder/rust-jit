# M013: observed compiler output accounting

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: intermediate compiler/string-operand outputs and final source-discovery
outputs share the same bounded budget. An immutable invocation output is charged
once in a discovery request, while an unrelated snapshot record is not charged
merely because it exists in the database.

The database provides a bounded coordinator-side observation window for actual
ExpansionRecord input reads. It preserves normal dependency collection and query
memo behavior, never reads files or executes macros, refuses starting inside an
active query or another window, and validates the unchanged input revision and
observed fingerprints before returning receipts. Missing records are not outputs.
Read receipts and accounting entries use stable keys, with explicit count/size
limits; no allocation address becomes a semantic identity.

SourceDiscovery observes compiler polls, validates produced/adopted record receipts,
and preflights cumulative token nodes, wire bytes and invocation count before
returning a produced output or integrating an admitted output. Explicit string
operand expansion charges its validated output before coordinator adoption. Repeated
reads, reproduced output and final admission match the same immutable descriptor
and are charged once. A conflicting descriptor is an expansion-conflict error.
A quota failure preserves the existing charged records and cannot publish a seal.
The maximum remains 2,000,000 token-tree nodes, 256 MiB wire and 100,000 outputs;
resource settings never increase automatically. Ordinary/declarative final output
still commits its charge after successful source integration.

Three controls prove combined intermediate/final token exhaustion, no duplicate
charge during compiler replay and final publication replay, and no charge for a
larger unrelated record adopted into the same immutable input universe. The test
driver returns typed failures for these controls; ordinary acceptance still unwraps
successful completion. No failure is treated as guest success.

Cross-domain run-wide accounting, procedural helper outputs, complete reduced
CLI/service limits, cancellation/supervision and broader discovery cycles remain
required. M014/M015/M016 impl/test/origin integrations, body-position user expansion,
comparison assertion operands, full quota/type/JIT/security/fixture/release evidence
and Linux acceptance remain pending. This component does not claim complete global
request accounting or complete M013 acceptance.

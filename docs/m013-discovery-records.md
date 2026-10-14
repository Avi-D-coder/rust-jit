# M013 immutable discovery record inputs

SourceDiscovery retains every successfully admitted ExpansionRecord in a deterministic
BTreeMap keyed by the exact ExpansionKey fingerprint. Records remain shared immutable
Arc values. Caller release cannot discard the tokens needed by later discovery work.
Exact-key read access is available while other producers remain pending; it does not
certify namespace absence or complete discovery. Refused replies never enter this index.

Admission preflights record count and allocated payload/metadata retention before
parsing or namespace publication. The retained allocation ceiling is 320 MiB, covering
the existing 256 MiB cumulative wire budget plus 64 MiB for metadata and allocation
capacity; the separate wire/header/job limits remain in force. Record-key collisions
or repeated admission refuse rather than replacing an accepted input. Full quota
boundary and publication-failure injection acceptance remains pending.

Seal exposes its immutable sorted dependency pairs. The sealed_expansions iterator is
available only after actual completed discovery and verifies that every retained record
has the exact key/descriptor pair in that certificate. Iteration follows stable key
order. Failed or incomplete discovery cannot export a completed input set. Published
seals still forbid later admission. This supplies record inputs for the subsequent
query-publication integration; it does not itself publish values in SourceDb.

Three controls exercise actual hygienic source expansions: retained Arc ownership after
caller release while another producer remains unresolved; exclusion of a mismatched
reply followed by valid admission; and sorted sealed input records with exact certificate
dependencies and post-seal refusal. No matcher, procedural helper, build tool or guest
operation runs from the record accessors. M013, generated outward macro bindings,
ancestry/invocation scheduling, query-database seals, complete MX027/MX028, release
and Linux acceptance remain pending. Validation is macOS ARM64 with Rust 1.99.0.

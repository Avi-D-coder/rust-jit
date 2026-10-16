# Ordinary indexed range lookup — bounded source prerequisite

October 15, 2026. LOCAL-024, T024/T025, architecture failure mode 8 and
E-R5-005 common-source repair. No feature/task acceptance is claimed.

Invariant: ordinary lowering already possesses actual syntax and BodyIndex, but
`syntax::thunks::id_for` scans every structural node each time it converts a
child into its indexed identity. Replace that repeated scan with one immutable
range index retained by the existing shared BodyIndex query result. This is
structural identity lookup only, never name/type/hygiene/runtime authority.

Root owns `syntax/body_index.rs`, `syntax/thunks.rs::id_for` and the existing
`syntax/node_lookup_tests.rs` oracle extension. Pattern and reference workers
keep their current files and consume the unchanged shared BodyIndex handle.
The query/memo framework, source dependencies, scope identity and verifier/VM
routes remain their existing owners.

Use sorted NodeId rows by actual relative start/end and original node ID.
Equal-range candidates preserve original first-match role selection, including
initializer administrative nodes. Prepare only when an entered body's ordinary
index is demanded; no body outside that demand is parsed or indexed. Count
vector storage and any capacity spare under the existing metadata ceiling.
The new rows are wholly derived from already explicitly encoded node fields,
so do not introduce a second semantic fingerprint or native-memory encoding.
Preparation is O(n log n), storage O(n); lookup is O(log n + equal-range rows),
with no parsing or whole-index scan per child phase. No measured speed claim.

Primary unchanged witnesses are the existing node-lookup oracle's return-nil.rs,
macro-nested_stmt_macros.rs, regions-infer-call.rs and match-range-infer.rs in
all official editions. Extend that existing external oracle to compare every
range candidate with the historical linear traversal, and preserve the existing
exact metadata boundary test on const-as-fn.rs. These structural controls are
not upstream runtime/full-fixture passes. The sole validator runs a frozen
successor asynchronously after its earlier queues; source review against all
eight failure modes and actual strict/milestone results are needed before any
integration claim.


## Frozen source review

Exact cumulative70 packet SHA-256 `1ddb4fe21f6d145e5afa2a73797cd2de1f0fb332264a285da6508511884c6112`.
Source-only; all Rust controls uncompiled/unexecuted. The eight-mode review is
recorded in `worktracks/evidence/2026-10-15-common-source-plans.json`.

- `crates/lrr/src/syntax/body_index.rs`: `583ca8a98bbbcea054bb603ea39a23cbff481a1bce3a356ba53486ed17e0fe8a`
- `crates/lrr/src/syntax/thunks.rs`: `3357dda8b5edff9a04073b2bb42abe72bdd180c96cb379b42774124231cf3f49`
- `crates/lrr/src/syntax/node_lookup_tests.rs`: `1b03c7fe7a2c8bb6963c4b0351ef6b776a05fcca0c1337bd261b7c879bf29720`
- `crates/lrr/src/syntax/mod.rs`: `601f866c25ad493f85d6c4516c0741b8093e2935a14eee346599bdec1192c868`

The existing unchanged-original oracle now compares every exact range candidate
with historical NodeId preorder. The existing const-as-fn initializer control
checks same-range literal/administrative identities and an absent location,
plus exact metadata limit behavior. No new substitute language fixture.

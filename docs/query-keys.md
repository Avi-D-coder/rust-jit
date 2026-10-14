# Current source/query substrate

Query keys and inputs use explicit `lrr/v2/` domain-separated encodings. Missing inputs
have a distinct fingerprint. Semantic header/body/scope identities exclude source
offsets; span attachments belong to the current snapshot. Parse keys include exact
content and the live semantics profile. Crate keys also include the profile.

The current implementation covers structural parsing, macro-free outlining, scope
buckets, and unresolved signature recipes. The Constant key/result currently serves
the synthetic dependency-graph acceptance tests; guest constant evaluation is not
implemented. The database validates dependencies recursively, caches deterministic
errors, and propagates non-cacheable cycle/variant failures to collecting ancestors.
There is no filesystem, process, network, effect, or JIT access in these providers.

The broader query catalog, expansion suspension, and revision-4 frontend integrations
remain pending. Memo byte estimates include owned result/dependency capacities,
key heap storage, and a 64-byte BTree-node allowance per entry. Parse estimates count source bytes plus
32 bytes per syntax element and owned diagnostics; this is a conservative accounting
model rather than a measurement of Rowan allocator overhead. The database is single threaded;
private parse trees are shared for ownership only, never sent to another thread.


T014 source publication stages a complete selected source/module-context universe
before replacing database inputs. Unchanged parse/outline handles are reused by
content/profile/module/cfg identity; changed outlines are all refreshed first.
Duplicate contexts, conflicting cfg/input records, and duplicate definition
identities fail staging. Deletions remove old inputs in the same atomic revision.
The source resolver must provide the complete context set; service/generated-source
integration is still pending and is not established by this component API.

Scope publication keeps its aggregate member record for enumeration, but ordinary
name-bucket queries depend on separate `ScopeBlockers` and `NameCandidates` inputs.
Missing candidate inputs are explicit negative dependency stamps. Adding a name
invalidates that bucket while preserving unrelated bucket reuse; changing a scope's
declaration-completeness blockers invalidates the lookups that depend on them.
Missing scopes still produce a typed internal-snapshot error. Deterministic failed
signature lookups are cached and revalidated like successful outputs. These paths
record real provider/reuse/invalidation counts in the existing query trace.

Current source-context handles have source/context-size bounds. T015 now separates
128 MiB query and parse budgets, with an additional 128 MiB source-input cap.
Eviction runs between runs, by oldest use counter then stable query-key order.
Retained result/inner parse handles pin memos. Source leases retain the exact parse,
snapshot, and attachment map; registered weak pins continue charging retired parse
storage until the last owner releases it. Attachment maps reject another snapshot,
so line edits cannot use stale offsets against a new source map. Shared source bytes
and registered attachment/parse allocations are counted once within these owners.
Line maps and span attachment payloads are charged to the parse budget; pin-registry
capacity is charged to the query budget. Checked total overflow and pinned caps
produce LRR-E-METADATA-LIMIT before a run starts. These are estimated capacities,
not measured RSS or exact allocator sizes.

Prepared publications now hold an immutable shared state with deterministic parse
and outline indexes. Publication registers a weak owner with the database. Active
input metadata and independently retained prepared-state metadata count toward the
query cap; retained source allocations count toward the source cap. Shared prepared
parse/outline trees are charged once across the registered states and current memos.
Providers read their ordinary immutable source/cfg dependencies before sharing a
matching prepared tree. A stale outline key whose cfg no longer matches the input
fails explicitly, rather than returning a prepared tree from another cfg universe.
Prepared refresh counters report actual parser/outliner work separately from query
provider counts; sharing a prepared tree does not perform a second parse.

Gate 01 now measures five initial source/query edit rows: selected and cold body
edits, a selected signature change, adding a missing name, and line insertion.
It records actual body/header/bucket fingerprints, provider and reuse deltas, and
dependency events. Parser/outliner refresh counts are reported separately. This
establishes source/query behavior only; executable thunks, regions, native artifacts,
and positional intrinsics still require the full T082 matrix.

Every returned query records weak ownership of its outer value and any inner
parse/outline/signature allocation. Retired values remain charged after memo
replacement; cleanup removes cache-only or dead weak registrations between runs.
Memo hits register newly returned ownership again. Returned input values similarly
remain charged after publication replaces their input records. Retained snapshot
metadata includes file indexes, crate/cfg records, and compile environment capacities,
in addition to its separately charged line maps and source bytes. Registry node
overhead is accounted with the same 64-byte BTree allowance as memo nodes. Private
allocation addresses only deduplicate accounting; they never become semantic keys,
serialized identities, or eviction-order inputs.

Gate 01 exercises a retained signature against actual measured totals and fails if
releasing it does not release the charged storage. Future dispatcher and expanded
query variants must use these ownership/run APIs. The installed-LRIR budget is a declared 64 MiB constant only; there is
no installed-LRIR cache yet. Macro-generated declaration/epoch publication must
integrate with M004–M012 before claiming revision-4 completion.

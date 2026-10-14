# M013 immutable captured source dependency aliases

M013 remains in progress. PreparedSourceInputs now accepts typed SourceDependency rows
through refresh_with_dependencies. Consumer/provider root sources must be captured;
canonical alias conflicts, counts and metadata are checked. The private immutable
SourceDependencies payload has explicit stable encoding and input tag 27. Per-unit
macro alias maps are shared through Arc rather than copied for every module index.
Existing refresh supplies an empty alias graph without adding dependency authority.

Discovery joins these aliases to the existing ordinary and macro/module engines.
Nonempty dependency graphs require all selected units in one immutable publication.
Original and generated macro imports select actual captured bodies. This source-world
alias input grants no build, procedural execution, runtime native or symbol authority.
Cargo-planner/world identity integration remains a separate requirement.

DiscoveryInput binds the captured alias graph fingerprint. Publication checks it
before appending derived inputs; pure DiscoverySeal verification reads the typed input
and suspends on changes. Alias edits invalidate positive/negative certificates even
when source bytes do not change. Row order does not alter the fingerprint.

Cross-unit nested dollar-crate output may retain the defining unit token root while
belonging to the caller declaration scope. Child catalogue admission now checks the
exact parent pending request and inherits its validated captured caller root. Header
plans consume that catalogue proof rather than inferring the caller from a token mark.
Public unattested header admission retains its root/context checks. Dollar-crate still
uses its explicit emitting definition unit. Existing wrong-owner/epoch controls pass.

Eight new controls execute renamed macro bodies, original/generated imports and
cross-unit dollar-crate in the tagged VM; refuse missing/conflicting providers; check
deterministic rows and invalidate stale certificates after alias edits. The preserved
origin witness is an implementation bug, not a pinned-backend contradiction.

Remaining work includes full Cargo/world/definition-unit facets, contextual origin and
selection invalidation, generated-body runtime macro contexts, type/quota/failure bounds,
full MX027/MX028 fixture/JIT matrices and ordered generated-test integration. These are
macOS ARM64 Rust 1.99.0 component controls. Full release and Linux acceptance remain pending.

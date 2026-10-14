# H001 edition identities — in progress

Selected live roots use Rust2024 and lrr-2024-v2. The pinned parser already admits
older macro-host source facets according to their recorded editions; this does
not authorize an older live root. A pure test using a captured planner graph
rejects an older selected root and preserves older dependency editions without
executing build tools or treating those dependencies as selected live source.

Raw r#gen declaration/call executes through the normal guest dispatcher. An
unrecoverable bare gen declaration now reports the pinned parser's Rust2024
parse-boundary diagnostic before namespace lookup. Declaration blockers trigger
this check; otherwise cold body errors remain deferred. Parser message rendering
is bounded and uses the actual original byte range.

E2411's current-key witness uses the normative CrateKey, including edition and
semantic profile. With identical source bytes it checks distinct parse, outline,
declaration, body structure, structural pattern/temporary index, phase and verified
recipe identities. Private structural test recipes are not executed as older live
units. Dedicated pattern/lifetime memoization and executable checkpoint restoration
belong to later tasks and have not been claimed passed by this key witness.

CLI completion, guest-panic, test-execution and test-listing reports now contain
live_edition and semantics_profile. CLI help identifies Rust2024. Macro definition
and dependency editions remain separate recorded facts; they are not rewritten to
match the selected root. The separate Rust1.99 macOS native oracle executes the raw
identifier source and rejects bare gen with the compiler's reserved-keyword error.
No selected live LRR runtime unit is compiled by that oracle.

H001 component acceptance is audited in [the cache audit](h001-cache-audit.md),
with explicit mandatory later integration in the ledger. H002/H003, complete E24 fixture
execution, generated-origin integration, native tiers and release acceptance remain
pending. Current milestone Gate03 is a component gate, never full acceptance.

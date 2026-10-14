# M013 completed child macro route snapshots

A child fragment now captures its import/export routes through the existing hygienic
resolver before its actual parent call position is retired. The resolver receives
the pending generated ancestors and the original invocation position. Snapshot
metadata binds the exact child record descriptor, captured owner scope, epoch and
explicit expansion origin. Parent completion checks those identities and the
existing child key/ancestry requirements before publishing the snapshot outward.

Snapshots retain immutable shared routes. Descendant snapshots move through each
completed parent, and the source provider contributes them to the existing macro
candidate graph alongside root routes. The module/import engine borrows those route
catalogues rather than cloning their content. No synthetic source tree or alternative
lookup engine is introduced. Macro expansion remains an explicit request action.

Snapshot payload and index capacity growth are checked and charged before semantic
completion. Count and allocation limits remain explicit. Failure cannot publish a
partial seal. This does not claim complete boundary coverage of all discovery limits.

Eight controls execute child-generated macro imports, qualified child exports,
imports of captured original exports and multiple child levels. They check the
actual parent lookup position, reject a later parent textual macro, preserve the
canonical child header in the selected request and require all three records for
the completed scenario. An unresolved child import stays incomplete. Four initial
missing-route failures and the mechanical borrowed-catalogue compile mismatch are
preserved. Root-route and recursive-discovery controls also pass in the targeted run.

M013 remains in progress. Qualified calls inside generated fragments, external
module/dependency/definition-unit chains, complete macro namespace query certificates,
generated-body runtime macro contexts, overlapping origins and remaining quota/failure
boundaries still require integration. Generated return/generic aliases and symbolic
type paths remain pending. Full MX027/MX028 CLI fixtures, later M014–M016, full E24,
release and Linux acceptance are not claimed. Validation targets macOS ARM64 with
Rust 1.99.0.

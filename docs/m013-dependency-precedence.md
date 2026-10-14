# M013 dependency refresh and source module precedence

M013 remains in progress. Ordinary PreparedSourceInputs refresh with a previous
publication now preserves its captured alias rows and revalidates them against the
new source roots. Initial refresh still supplies empty rows. Explicit
refresh_with_dependencies replaces the graph, including intentional removal with
an empty vector. Bounded row copying preserves deterministic canonical identities.

Macro import routing now consults the existing combined captured module catalogue.
A disclosed same-named local module shadows a dependency alias in relative paths;
absolute dependency paths retain their explicit external meaning. Existing module
alias resolution and visibility checks remain in use. Generated imports no longer
choose a dependency merely because their own fragment lacks the original local
module declaration. Macro import facet verification uses the same complete namespace
path interpretation rather than a fragment-local precomputed target.

Seven new controls execute actual captured macro bodies through the tagged VM after
refresh, local relative paths/imports and generated imports, and explicit absolute
paths/imports. Explicit removal stays incomplete and cannot seal. Three baseline
cases failed: refresh erased aliases, original collision imports were refused, and
a generated collision import executed the foreign body (137) instead of local body
(149). This is an implementation repair, not a pinned-backend contradiction.

Full Cargo/world/facet integration, complete contextual origin invalidation,
generated-body runtime macro contexts, type/quota/failure boundaries and full
MX027/MX028 fixture/JIT matrices remain mandatory. Test/generated catalogue work
follows fixed M014–M016 order. These are macOS ARM64 Rust 1.99.0 component controls;
full release and Linux acceptance remain pending.

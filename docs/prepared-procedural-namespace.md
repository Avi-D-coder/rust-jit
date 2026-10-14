# M010 prepared procedural namespace facets

`MacroSession::export_catalog` exposes immutable, owned export facts from libraries
already admitted and loaded under macro authority. It reads frozen inventory and
exact preparation producer-unit/edition bindings; it does not query the server,
load another library or execute a macro. A different consumer context refuses.
The catalog remains readable after a cached macro fault closes the helper.

Each opaque namespace definition has a stable encoding of producer unit, compiled
artifact identity, producer edition, exact SDK export name and macro form. The
catalog maps that definition back to `MacroIdentity::Procedural`. Modified
provenance and wrong invocation form refuse. Unknown definitions remain outside
the procedural catalog, so declarative and builtin classification stays explicit.
Canonical source names use the existing pinned identifier adapter, including raw
keyword names. The catalog bounds rows and retained metadata before allocation.

`SourceMacroRoutes::from_procedural_catalog` contributes public root exports and
module membership to the existing source route graph. Source imports, crate
aliases, reexports and visibility continue through the existing namespace and
module worklists. This introduces no alternative resolver or frontend. Graph
construction produces a new candidate graph; it does not publish partial query
results. Compiled export inventory is a macro-export facet and contains no inferred
helper attributes or runtime native symbols.

Actual controlled macOS preparation and native Rust 1.99.0 acceptance exercise
imported derive/attribute aliases, qualified function exports and raw names.
Selections retain exact compiled artifacts and kinds. Pure catalog inspection has
zero macro invocations. The original item-request fixture now resolves its positive
targets through these source routes before assembly, query suspension, external
execution, immutable adoption and worklist advance. Older producer editions remain
attached to definitions; cached-fault cases retain the same catalog without retry.

Automatic item query orchestration, builtin/helper attribute classification,
generated-source rebuilding, expansion-frontier sealing, discovery and guest
execution remain pending. This component does not establish complete MX018–MX020,
M010, release acceptance or Linux support.

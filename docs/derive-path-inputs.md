# M010 derive paths from owned tokens

The adapter parses one parenthesized derive argument forest into ordered owned path facts. It partitions only the top-level comma separators, then feeds each original token slice to the pinned syntax bridge and parser. The result must be a PathType without generic arguments, type anchors or parenthesized arguments. It neither prints/reparses an expansion nor uses a HIR SourceDatabase.

Each result retains absolute qualification, ordered segment spellings (including raw names) and the original first/last span keys. Empty derive lists and trailing commas are admitted; empty interior entries, malformed paths, generic paths and non-path types fail. Path count/depth and metadata sizes are bounded. Resolving those paths remains a separate namespace demand.

Adapter acceptance checks source ranges, lexical ordering and negative syntax. The actual Rust1.99 fixture accepts a derive list combining a renamed imported macro, an absolute raw-name macro and a crate-qualified reexport, using the explicitly prepared synthetic proc-macro crate. These prove syntax/qualification admission, not complete generated catalog or guest behavior.

Connecting source attributes to the lossless argument forest, automatic exact-export helper metadata, replacement/append worklists and full MX018–MX020 remain pending. M010 remains in progress.

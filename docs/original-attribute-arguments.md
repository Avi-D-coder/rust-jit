# M010 original attribute argument selection

AttributeInput records the pinned AST's token-tree argument range, including nested active cfg_attr metas. argument_forest selects that exact group from the caller's original captured token forest. It checks the frozen parse content, source identity, owning item/meta ranges, delimiter spans and edition, and rejects missing or ambiguous groups. The internal caller remains responsible for supplying the authoritative original captured forest, rather than a generated output disguised with original spans.

Selection copies the original token trees and only their referenced span records and complete context ancestry. It does not construct arguments from TokenRecipe display text. macro_argument_contents changes only the outer delimiter to invisible for the procedural attribute ABI; nested groups, raw identifier flags, lifetime/punctuation spacing, literal spelling and source/hygiene keys are retained.

The original-source test selects derive and transforming arguments nested in cfg_attr and checks retained records against the original forest. Stale source, inactive item and bad index reject. The actual matched-server attribute_tokens fixture returns the supplied argument tokens as replacement items; the generated function executes identically under native Rust1.99, while the discarded source body's unresolved call is never checked by the native oracle.

This establishes argument transport and one actual replacement boundary. General attribute-removal/item reconstruction, sequential replacement/derive append, automatic helper declaration metadata, discovery/catalog and full guest fixtures remain pending. M010 and the project are not complete.

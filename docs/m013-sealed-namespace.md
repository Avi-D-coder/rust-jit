# M013 certificate-dependent namespace queries

Completed ordinary value/type discovery now snapshots the actual NameNamespaces
lookup outcomes into its immutable discovery input. It enumerates the existing
finite member-name universe, preserves definitions, ambiguity, inaccessible routes
and module-root outcomes, checks graph frontiers and caps names/metadata before
publication. A requested name absent from that completed universe is a certified
absence. The snapshot uses the captured owner scope, namespace and edition.

QueryKey::SealedName carries the exact seal key and bucket owner/name. SyntaxProvider
first fetches and validates the discovery certificate through the ordinary query
tracker, then reads its immutable namespace snapshot. Wrong scope/namespace domains
are refused. The positive and negative lookup values both depend on the certificate
query/input; replacing the source universe invalidates them. A missing certificate
suspends without a semantic memo. Lookup outcomes have explicit stable encodings and
checked metadata accounting; no matcher, native helper or body lowering runs here.

Six controls exercise actual generated imports and certified absence, ambiguity
without arbitrary selection, wrong-domain refusal, missing-name invalidation after
source replacement, type/module outcomes, imported module aliases and refusal to
certify a macro name while macro namespace routing is unfinished. The existing
publication controls also pass. Macro snapshots remain unavailable until their
textual/path/import/export integration is complete; absence is not substituted.

M013 remains in progress. Generated declaration/body query contexts and full runtime
source integration remain required, along with macro namespace import/export and
qualified/contextual/definition-unit routing, external-module chains, overlapping
expansion-origin cases and complete quota/failure boundaries. Full MX027/MX028 and
E24/release gates remain pending. M014–M016 follow in fixed order. Validation is
macOS ARM64/Rust 1.99.0; Linux acceptance remains deferred.

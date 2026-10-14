# M013 owned generated item-call facts

The pinned item-output adapter now retains each top-level generated macro call's
owned path and argument token forests, delimiter call-site span, and path-context
edition. Extraction uses the existing pinned token-tree/syntax bridge and origin map;
rendered output text is never reparsed into an alternative input token stream.

GeneratedItems projects these adapter forests through the checked schema2 wire
boundary and binds each call to its containing canonical GeneratedItemKey. Repeated
identical call spellings remain distinct through structural item ordinals. Qualified
paths retain identifier tokens, punctuation and original span/context records. Argument
groups retain nested delimiters, literal spellings and punctuation spacing.

At most 100,000 raw top-level calls are retained per item output. Calls and declarative
bodies share a checked cumulative 256 MiB retained-wire budget. Vector reservation and
schema decoding failures remain typed errors. Full request-wide quota acceptance is
still pending; these controls do not assert boundary coverage.

These are raw syntax facts, including facts under inactive cfg. Discovery must apply
captured cfg and resolve the path with its owned contexts before scheduling an expansion.
This component executes no matcher, procedural helper, build tool or guest operation.
GeneratedMacroCatalog and SourceDiscovery retain incomplete producers until that
scheduling/resolution integration is complete. Calls inside function/block bodies are
not enumerated here; the broader nested-declaration scan remains M014 work.

Four controls exercise actual hygienic macro output: argument literal/nested-group
origins and canonical header identity; distinct structural keys for repeated calls;
qualified path tokens and origins; and exact argument-subtree equality including
separated punctuation spacing. The invalid brace-call fixture's extra semicolon was
corrected to valid Rust item syntax, with its failing log preserved.

Continue M013 generated-fragment cfg/order/path/hygiene resolution, enclosing ancestry,
input/seal query publication, imports/transformations and complete MX027/MX028.
M013, full LRR fixtures and release acceptance remain pending. Validation is macOS
ARM64 with Rust 1.99.0; Linux remains deferred.

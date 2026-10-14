# M013 original-source declarative demands

SourceDeclarative retains the exact immutable PreparedSourceView and builds the
existing SourceMacroIndex with pinned root hygiene facts. It resolves an enabled
original item invocation by its captured structural producer identity. No file is
read from disk and no executable body is entered.

A bare textual macro selection uses the existing source order and hygienic resolver.
Unknown names, qualified/imported routes and earlier unresolved producers return
NeedInput; an absent or cfg-disabled invocation identity refuses. An attribute
producer takes precedence over its annotated invocation. Import and generated
scope integration remains pending rather than being treated as successful absence.

Once a definition is selected, the retained pinned AST identifies the exact body
and argument token-group ranges. The pinned source-token adapter supplies the
original source identity, edition, ranges and contexts. Selected owned groups use
those origins, with an explicit 32 MiB source token-wire admission limit. Rule
parsing occurs only when the ready request is explicitly expanded. The matcher
receives the exact source definition owner and expansion identity and retains its
two-million-token/32-MiB output limits. No build tool or procedural helper runs.

The request key binds selected definition, original invocation identity, actual
input wire digest, caller root context, definition/call editions, item grammar,
and the coordinator-supplied frozen compile-environment and engine fingerprints.
The caller must obtain those fingerprints from the run's frozen identities.
The request exposes its immutable key and cannot substitute input or definition
payloads after resolution. Source changes produce distinct captured token identities.

Four controls exercise actual macro argument substitution with captured origins,
lazy malformed-rule failure, typed pending unknown/preceding producers and inactive
cfg definitions, source shadowing and snapshot-sensitive output identities. An
ordinary cold body containing unresolved types and calls is retained without entry.
These are original-source expansion component controls. They do not certify
MX027/MX028, full producer quotas, source/generated import integration, discovery
retry, scope-seal publication, generated execution or full LRR fixtures. M013 remains
in progress. Validation uses macOS ARM64 and Rust 1.99.0; Linux is deferred.

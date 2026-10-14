# M013 generated calls in captured enclosing scopes

SourceDiscovery now resolves a generated bare-name call through the existing
MacroDeclarations resolver with the captured original SourceMacroIndex as its
outer scope. The outer visibility position is the original parent invocation,
not a generated token offset or the macro-definition position. Inline-module
parent scope visibility remains frozen by the existing TextualScopes chain.

Resolution joins pinned owned hygiene facts and explicit expansion-origin tables.
The ancestry join validates supplied parents in deterministic depth/key order and
rejects conflicts; it does not infer producer parents from syntax marks. Local
and inherited candidates are selected together by the existing hygiene/origin
rules. Missing producer facts still suspend before absence or shadowing is selected.

An inherited selection creates an exact immutable request from the retained
captured original body or previously published generated catalog. Actual generated
argument forests cross the schema-checked wire boundary with their spans, contexts,
punctuation spacing and edition. The selected definition, input digest and actual
call-site context/edition must match the request. Captured original bodies remain
unparsed as macro_rules until explicit request execution. Polling grants no tool,
procedural-macro or native authority.

Seven controls exercise real matcher output and resolution: an original macro
emits a function satisfying an original import; a local generated macro shadows
an original definition; a later original definition stays unavailable; a prior
outward generated definition consumes actual generated arguments; lookup uses
invocation position rather than definition position; inline-module inheritance
publishes the resulting header in the captured module; and malformed original
rules remain lazy until explicit execution while preserving the pending demand.

M013 remains in progress. Recursive generated producer/binding/import integration,
external-module source chains, qualified/contextual paths, definition-unit routing,
immutable query database publication, complete quota/failure boundaries and full
MX027/MX028 remain required. Nested declaration scans follow as M014. Full E24,
release and Linux acceptance remain pending. Validation uses macOS ARM64/Rust 1.99.0.

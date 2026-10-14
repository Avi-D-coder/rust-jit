# M013 owned generated declaration and body queries

Completed discovery now publishes generated syntax inputs and canonical declaration
rows alongside its records and certificate. Each output carries the captured original
caller scope/configuration. Active declarations map the pinned replayed syntax ranges
to GeneratedItemKey fingerprints; body handles use those canonical definition IDs.
Headers, source locators, declaration contexts and body handles are immutable derived
inputs. The base publication API cannot inject generated syntax. Publication checks
matching certificate dependencies, duplicate/immutable conflicts and metadata budgets
before changing the database revision.

Generated parse identity is an explicit stable encoding of the expansion descriptor,
so identical display text from different outputs cannot share a source identity.
No display text is published as a SourceBlob. A ParseFile query reads the owned
GeneratedItems input, validates its discovery certificate and replays the pinned
syntax-event stream into the existing syntax tree. Signature, body-structure and
thunk queries then use the existing providers. No matcher, native helper or rustc
runs in a provider. Source replacement invalidates the certificate dependency and
can return a typed rediscovery demand during memo validation.

Generated syntax/body/header/hygiene capacities and retained caller-scope metadata
have checked accounting. Inputs shared by value and type certificates retain the
first compatible immutable syntax owner rather than conflict because their namespace
certificate differs. Query parser arenas remain ordinary memo-owned parse values.

Four controls cover real generated signature/body queries and tagged-VM execution
returning 7 while a cold invalid body has no BodyStructure memo; distinct canonical
parse/body identities for identical generated text; certificate-dependent parse
invalidation; and compatible reuse across value/type certificates. Related recursive,
namespace and publication controls pass. The first stale-query assertion expected a
ready error; actual dependency validation suspends for rediscovery. The corrected
control asserts that specific demand, and the failed observation is preserved.

M013 remains in progress. Original callers must still resolve and invoke generated
callees through the complete runtime source context; generated-body macro contexts,
macro namespace import/export and qualified/contextual/definition-unit routing,
external-module chains, overlapping origin cases, complete quota/failure boundaries
and full MX027/MX028 remain required. M014–M016 follow in fixed order. Full E24,
release and Linux acceptance remain pending. Validation uses macOS ARM64/Rust 1.99.0.

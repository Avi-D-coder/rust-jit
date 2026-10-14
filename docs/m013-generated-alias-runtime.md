# M013 generated alias runtime

The existing demand-driven AliasNormalizer now locates generated declarations
through immutable GeneratedSyntax inputs. It requires the exact parse identity,
canonical generated definition identity, captured owner scope and matching header
profile. ParseFile validates the owning discovery certificate before generated
declarations can complete normalization, including nominal completion. Original
source catalogue validation remains in place.

Alias chains retain each definition's own declaration scope and canonical identity.
Existing bounded cycle detection, deferred generic aliases, nominal outcomes and
typed pending-producer outcomes remain in use. Providers only read owned syntax;
normalization grants no build, native or macro execution authority.

Five controls execute a generated parameter alias through an import, a chain crossing
original/generated/original declarations, and an explicit generic alias argument
inside a generated function. A cold generated alias cycle stays unqueried. A reached
cycle produces LRR-E-TYPE-ALIAS-CYCLE before the argument function's BodyStructure
query. The initial four failures exposed the original-only catalogue lookup; that
failed observation is retained. Existing original alias and generated call controls
also pass in the targeted run.

This is M013 component work. Generated alias return descriptors, generic alias
applications and other symbolic type paths still require integration. Generated-body
macro contexts, macro import/export and qualified/contextual/definition-unit routing,
external-module chains, overlapping origins, remaining quota/failure boundaries and
full MX027/MX028 acceptance remain pending. M014–M016 follow in order. No full
fixture, release or Linux acceptance is claimed. Validation uses macOS ARM64 and
Rust 1.99.0.

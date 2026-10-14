# M007 macro glob contributions and lookup

Original source glob imports now create weak macro member edges after resolving
and checking their source module paths. Names come from disclosed explicit macro
and import facts; globs preserve names and original definition identities. The
builder enforces source import, derived header, path and metadata limits.

Caller-aware glob lookup uses per-walk owned candidate tables. Explicit macro
bindings and named reexports take precedence over weak glob candidates. Weak
visibility intersects the import's declared visibility with the member's scope,
so public globs do not widen crate-only members. Diamonds deduplicate definition
identity and multiple distinct origins remain ambiguous. Pending producers
propagate without caching incomplete outcomes.

The dependency graph is partitioned into strongly connected components using
iterative traversal. Dependencies evaluate first. A cyclic component updates
simultaneously, preserving independence from traversal order. Producer-complete
empty cycles settle empty; a rooted glob cycle propagates its unique origin. A
repeating strong/weak choice state returns an explicit cycle failure rather than
choosing an origin. Stable state fingerprints use explicit encodings. Lookup
tables are ephemeral and impose checked metadata, candidate and work bounds.

Three source tests cover propagation, named shadowing, ambiguity, visibility,
diamonds, empty/rooted cycles, pending/publication transitions and conflicting
strong/glob cycles in both declaration orders. The native macOS Rust1.99 fixture
executes forwarded, explicitly shadowed, cyclic and public-member calls. It
requires the exact ambiguous_glob_imports diagnostic for external ambiguous
reexports and E0603 for a private member. The initially incorrect E0659 expectation
is retained in artifacts/macos-first/m007-globs-before-diagnostic-fix.log; this is
a test expectation correction, not a language-profile change.

This component contributes macro members only. Original module glob members
now integrate through [module glob resolution](module-glob-resolution.md).
Ordinary item members introduced by globs, generated-source/producer completion, membership seals,
relative invocation AST integration, local bindings and guest VM dispatch remain
pending. Candidate tables do not establish complete namespace membership or a
finished release.

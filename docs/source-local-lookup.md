# M007 original source local-name lookup

EnteredMacroIndex builds an immutable SourceLocalIndex from the exact retained
source/body, owned binding recipes, macro-definition ribs, producer frontiers and
typed pattern resolutions. Unknown candidate names yield name-specific NeedInput;
Constant candidates do not introduce locals. Explicit bindings cannot be
reclassified as constants. Unknown resolution IDs and duplicate resolved names
requiring pattern equivalence are rejected before exposing an index. The index
borrows its entered parent and shares the admitted pinned hygiene facts by Arc.
No query, expansion, compilation, filesystem access or guest effect runs here.

Source byte boundaries map to checked logical positions with separate per-record
slots. Multiple destructured/parameter bindings can share a visibility boundary
without colliding. Bindings are visible at that boundary; macro ribs and producer
frontiers become visible after their declaration start, so the current call is
not its own prior producer. Block children capture their parent's source position.
Named-function entry builds an isolated local root. Public lookup checks exact
source/body revisions, entered ranges and opaque syntax before consulting locals.

LocalScopes now retains known-name unresolved rows separately from arbitrary
producer demand. Lookup considers only the requested name and pinned local
context, preserving shadowing and exact emitting-definition rib traversal. No
replacement hygiene algorithm or item/type checker is introduced. Pattern
resolutions and expansion-to-definition metadata are explicit owned inputs;
their service/query production remains later integration work.

Four source checks cover shadowing initializers, destructuring, parameters,
block ancestry, name-specific demand, constant exclusion, exact selection guards,
prior producers/current-call visibility, pattern equivalence requirements and
named-function isolation. The existing native collision fixture now also uses
original-source binding discovery and ribs with actual pinned marked tokens.
Selected original initializer values are read from the fixture source and match
three native observations: definition-site x, caller argument x and outer x.
Its generated-local comparison remains the separate supplied-fact kernel check;
this component does not claim generated-source publication or VM execution.

Receiver/loop/match/let-chain bindings, resolved alternative-pattern equivalence,
closure capture, generated-local publication and
query/service consumption remain pending. Checked local row/table metadata and
existing record budgets bound construction; M012 request-wide accounting remains
required. M007 and release remain incomplete. Actual macOS evidence:
artifacts/macos-first/m007-source-locals-checkpoint.json.

Linked-wrapper source/native acceptance is now recorded separately in
[linked-local-lookup.md](linked-local-lookup.md).

Original for-pattern scopes now have separate source/native acceptance in
[for-binding-scopes.md](for-binding-scopes.md).

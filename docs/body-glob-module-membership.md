# M007 module membership supplied by entered-body globs

An ephemeral synchronous table resolves the module targets of original retained
body imports in the selected lexical scope and its ancestors. Unselected child
scopes do not run namespace resolution. Each round reads the preceding round, so source iteration order
cannot decide a cyclic result. Scoped named and glob indexes preserve lexical
parents; complete empty cycles settle as candidate absence. Repeated nonstable
states fail with a deterministic diagnostic. The state digest uses explicit
stable encodings for identities, routes, visibility, pending inputs, denial
witnesses and macro-facet facts. No table or absence is persisted across source
publications, and no namespace seal is claimed.

A module candidate retains canonical unit/path identity and its original access
route. Canonical identity deduplicates module diamonds; the access route resolves
later members without losing a public reexport's permission to cross a private
ancestor. Glob target queries and qualified body macro calls share this table.
All source-module traversal still uses the existing caller-aware module registry.
No synthetic body module or unrelated namespace binding is published.

An import's source lookup excludes that import's own contribution. This permits
same-name imports from outer source modules and from other globs. Other unresolved
strong imports reserve their module names: a mutual strong cycle cannot seed
itself through a same-name weak glob. A proven macro-only import does not reserve
the module namespace, and that facet fact propagates through forward aliases.
Transforming import facts and unresolved source-member inputs retain NeedInput.
Private glob members are filtered independently from inaccessible glob targets.
Different canonical module identities retain ambiguity even if their macros
would happen to point to the same original definition.

Four new source acceptance checks exercise chained glob targets, forward and
same-name module imports, qualified calls, a macro/module spelling collision,
escaped public reexports, empty glob cycles, an unseeded strong alias cycle, and distinct-module ambiguity despite equal
macro definition identity. A demand regression proves an invalid import in a
child scope is diagnosed only when that child's invocation is requested.
The macOS Rust1.99 native fixture compares actual pinned-hygiene selections with
original source definition identities and executes four calls. A separate native
strong-cycle witness is rejected with E0432; an ambiguity witness with the same
original macro definition in two modules is rejected with E0659. This is candidate-resolution evidence,
not guest VM execution or complete native/guest diagnostic equivalence.

Import count, table/index/route sizes, candidate admission, state-history sizes,
route depth and module-table walk work are checked. Candidate copies follow
metadata admission. Tables are temporary; request-wide M012 accounting, body item
and module catalogues, source local-binding ribs, generated publication,
membership seals and guest VM integration remain pending. M007 and the release
remain incomplete.

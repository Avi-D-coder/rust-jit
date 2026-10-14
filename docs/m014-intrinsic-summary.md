# M014 intrinsic no-item proofs and declaration publication

Task and invariant: cold intrinsic syntax may retire a declaration producer only
after the existing resolver selects a compiler-owned definition and the argument
parser proves it contains no declaration, nested macro or transforming annotation.
The predicate, formatting and ordinary operations must remain unevaluated.

The complete source-worklist gate exposed two CLI regressions: runtime assertions
became unresolved declaration producers. The failed gate and its command output
are retained. Pure declaration lookup now has an explicit API; executable lookup
keeps its body-entry restriction. The declaration API checks the captured source,
module path, lexical readiness and namespace identities. Compiler runtime facets
come from the configured catalog. User definitions and unresolved argument
producers cannot acquire a no-item proof.

Discovery publishes those proofs alongside its certificate. Publication checks
source, body identity, invocation bounds, compiler identity and actual parsed
arguments, with bounded per-source parser caching. Stale source refuses the old
publication. Completed non-item declaration fragments and their exact expansion
records also participate in dependency validation and publication; they remain
outside item-syntax preparation. All publication uses the existing database and
coordinator boundary.

Five new controls cover an unevaluated cold predicate, unresolved nested argument
syntax, a user `assert` definition emitting an impl, stale proof publication and
exact non-item record publication. Both CLI regressions pass in the isolated
draft, along with all 20 CLI discovery controls. Broader integrated validation
is required before a new milestone is accepted.

The last frozen checkpoint remains 1733 host/doc passes. M014 is in progress.
Intrinsic arguments containing declarations still need audited argument scanning;
procedural/qualified and transformed/dependency integration, mixed editions,
complete graph/facade impl seals and reached symbolic selection remain pending.
Full rustc fixture, guest JIT, release and Linux acceptance remain incomplete.

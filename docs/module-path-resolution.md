# M007 retained module paths and prefix privacy

The source candidate graph now retains declared module visibility, original
module-alias targets and their import origins. Module aliases remain available
for direct qualified macro calls after construction. Dependency aliases supply
private root bindings when no explicit source binding shadows them.

Iterative continuation frames resolve original alias targets from their import
origins. Every accessed module prefix checks caller visibility. Only the alias
result returns to the original caller for remaining path components. This allows
a public module to be reexported through a private ancestor without reopening
that ancestor to external callers. Private aliases remain private, and public
aliases cannot widen a private final module binding. An alias cannot hide a
private final macro binding from the E-M007-011 reexport-width check.

Alias targets retain their original spelling routes instead of flattening away
privacy provenance. Macro reexport targets also retain these routes; canonical
binding lookup happens after module access resolution. The raw candidate API
resolves aliases without claiming access. Caller-aware lookup checks prefix and
binding access together. Source identity, defining unit and edition survive
resolution. Source catalogs and the retained registry have checked metadata and
path limits; the walk has bounded iterative frames and explicit cycle failures.

Two source regressions cover direct calls, public escapes, private prefixes,
private aliases and macro widening through an alias. The native macOS matrix
executes permitted paths, requires E0603 for forbidden prefixes, E0365 for module
widening and E0364 for macro widening. The earlier selected-expansion oracle now
also calls forward dependency/module/raw aliases directly; six selections and
expansions from full original token forests match actual native execution.

These are disclosed original source facts, not completed membership seals.
Original macro/module globs now integrate through the source candidate graph;
generated epochs, producer completion, undisclosed cold-body exports,
relative invocation AST integration, local bindings and guest VM dispatch remain
pending. No full runtime privacy/security or Linux support claim is made.

Direct disclosed-module lookup now has a separate typed result: canonical unit
and module components plus the final binding visibility, absence, or an
inaccessible module witness. Empty paths select disclosed crate roots. Unknown
units remain absent. Alias traversal preserves import-origin access and width
checks. Macro lookup uses the same iterative module walker before appending its
member name. Glob-source checks call the module walker directly.

Two additional acceptance checks exercise actual parsed public escape/private
alias routes, empty roots, absent modules/units, and the full 256-component
module boundary with checked rejection at 257. This removes the synthetic macro
marker from glob-source validation and supplies the typed membership needed by
module glob propagation. Module glob members now integrate through
[module glob resolution](module-glob-resolution.md); membership seals remain pending.

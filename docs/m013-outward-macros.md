# M013 outward generated macro bindings

GeneratedMacroCatalog exposes cfg-filtered root declaration facts in emitted textual
order: exact GeneratedItemKey identity, name, context, explicit source owner and
keyword edition. GeneratedHeaderPlan now identifies each macro frontier by that
canonical header key, rather than merging it with all other producers from the same
invocation. Unresolved modules, impls, attributes and generated calls remain opaque.

When a fragment has no remaining producer beyond ordinary imports and validated root
macro declarations, SourceDiscovery publishes those declarations through the existing
SourceMacroIndex, MacroDeclarations, ExpansionAncestry and TextualScopes operation at
the original invocation position. Its pinned owned hygiene tables are extended with
conflict/count/metadata checks; no replacement normalization algorithm is introduced.
Macro declaration frontiers are released only after this publication succeeds.

SourceDeclarative retains a shared catalog for each registered generated definition.
A later original invocation resolves through the existing textual/hygienic index and
constructs its key from the selected generated identity and the real original argument
tokens. Explicit demand dispatches that body through the catalog's pinned matcher.
Rule parsing remains lazy. Original declarations after the producing invocation keep
their source-order priority. Multiple definitions emitted by one expansion retain
emitted order, not hash order. Inactive cfg definitions are not published.

Five controls use actual source expansion: an earlier macro emits an import and a
macro required by a later original call, which produces the imported function with
canonical identity; emitted duplicate names preserve textual order; a later original
macro shadows the generated binding; malformed generated rules fail only on later
demand; and cfg-disabled output leaves the later call incomplete. These controls
exercise the MX027 discovery shape, but do not claim the full LRR runtime fixture.

Remaining M013 work includes generated invocation scheduling within fragments,
enclosing/cross-fragment ancestry beyond these original root invocations, macro exports
and contextual/definition-unit import routing, nested/module/attribute integration,
query-database input/seal publication, full producer quota boundaries and publication
failure injection. Full MX027/MX028 and release acceptance remain pending. Validation
is macOS ARM64 with Rust 1.99.0. Linux remains deferred.

# M007 explicit original container entry

The entered-body catalogue retains a separate ContainerKey and exact source
range for module, trait, impl and extern-block containers. Keys encode the
source definition, content digest and item kind with explicit stable tags.
Explicit entry validates the retained source selection, parent body or container,
declaration membership, kind, exact range, content and attribute eligibility.
Macro templates cannot enter through this API.

Container entry collects immediate cfg-enabled headers and lexical facts from
the immutable source lease. Executable bodies and nested containers stay opaque
until explicitly entered. Recursive module entry retains the declared module
path. Trait, impl and extern headers supply associated header context, including
generic parameters and predicates, through owned token recipes. Checked sizes
and allocation admission bound context and header collection. This catalogue
does not publish a body-local module into the global macro namespace.

Two source acceptance checks cover ordinary container kinds, recursive module
paths, associated context, executable opacity, stable declaration identity across
changed content, stale source/parent/content rejection, transforming attributes
and template rejection. They are source-fact evidence; no executable VM or
native comparison is claimed for this component.

Explicit function entry from these containers now has separate source/native
acceptance (see container-body-entry.md). Body-local namespace publication,
local-binding ribs, generated publication, namespace seals, guest VM integration
and request-wide M012 accounting remain pending. M007 and release remain
incomplete. See artifacts/macos-first/m007-container-entry-checkpoint.json for
actual macOS acceptance results.

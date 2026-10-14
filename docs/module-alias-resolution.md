# M007 source module aliases

Original source macro catalogs retain declared module paths and named import
facts. A deterministic worklist discovers which imported names refer to modules,
including forward chains, dependency roots, `crate` aliases, grouped `self`
imports and raw module names. It records module aliases independently of macro
routes: the same imported spelling may contribute to both namespaces.

A job waits on the first unknown module prefix. Publishing that alias wakes only
its dependent jobs; aliases store their resolved target, avoiding recursive
resolution and repeated full passes. A 400-link forward chain exercises this
behavior. Distinct targets for one alias and conflicts with a declared module
fail explicitly. Unresolved module references never become a final macro
absence. Conversely, a producer-complete macro reexport cycle with no originating
macro remains empty even if its names are not modules.

Macro import targets are rewritten through resolved module prefixes before the
candidate graph is returned. Construction remains atomic: errors discard the
new graph. Paths have at most 256 components; the retained worklist, module and
alias keys and candidate routes have checked cumulative metadata admission.
There are at most 100,000 named import jobs and a bounded wakeup count. Definition
identities, defining units and source editions are preserved.

The new native macOS Cargo fixture contains a provider with two distinct macros,
forward dependency/root/module aliases, a raw `r#gen` module and a final macro
rename. Source catalogs use actual adapter-owned hygiene facts. Selected
macro identities locate the rule bodies inside full original token forests,
preserving unit/content/path and source offsets. Actual pinned expansions match
the native program's `[17,17,17]`; no expected guest output is substituted.

This is candidate resolution, not a visibility or completeness seal. Globs,
attribute/textual producers, dependency/module collisions, generated source and
undisclosed cold-body exports remain pending. The guest VM and native-world macro
export facets are not claimed implemented by this component.

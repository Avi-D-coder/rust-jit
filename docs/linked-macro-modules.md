# Captured external-module macro source

PublishedMacroIndex records typed ModuleSource inputs from the original source
outline. Linking a selected child requires the exact parent selection, a known
module demand, the child's matching logical root/unit/profile/cfg and a view
from the same immutable prepared publication. Matching bytes from a different
publication do not establish a coherent source family. Name facts must agree
with the containing source unit.

LinkedModuleIndex retains the selected child and borrows the captured parent.
It joins child textual candidates with parent visibility frozen at the module
opening. Recursively linked children extend this ordered source-scope chain,
with checked depth/candidate/allocation bounds. Lookups validate the complete
ordered source-selection chain. The existing pinned-name and expansion-origin
selector handles the combined candidates; no alternate resolver is introduced.
Linked entered-body lookup adds its exact body scope before the child source
and captured ancestor scopes. Original unresolved parent indexes stay explicit
input demand; the linked view is the complete selected file environment.

Linking and lookup never read a file or execute a macro. The caller supplies
already captured source selections from the authorized source preparation/
resolver. Path/default-file/attribute-path resolution, directory seals and
source-wide catalog publication remain separate integration requirements.
This component proves coherent selection and lexical inheritance, not that
arbitrary prepared source contexts establish correct path resolution.

Three regressions exercise recursive source/body capture, consistency of every
inherited identity row, and rejection of stale,
wrong-context and foreign-publication links. The native three-file macOS test
selects macros from the complete original token forests with their original
unit/content/path and offsets, then uses the pinned MBE. Early/late child and
grandchild functions and root source select values matching native
[17,53,41,61,29]. Source file selections are supplied explicitly. Guest execution,
local binding discovery, generated epochs and import/export catalogs remain
required. M007 remains in progress.

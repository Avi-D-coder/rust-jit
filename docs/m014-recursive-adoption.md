# M014 completed-child publication and declaration subtrees

Task and invariant: adopt actual generated declarations in lexical order while
retaining unresolved child producers. Discovery must not execute cold ordinary
statements, treat a rejected reply as completion, or issue an impl seal from one
declaration subtree.

The existing source and generated macro indexes now assemble completed-child
publication on cloned state. They validate the complete key, parent inventory,
remaining producers and registration conflicts before installing it. Outward
definitions retain actual body owners, hygiene facts and explicit ancestry.
Statement-root definitions become available to following siblings; definitions
inside expression blocks stay local. A source-order index selects the next
pending generated producer without rescanning the whole inventory.

`DeclarationFragment` assembles a subtree for a coordinator-held discovery ticket.
Polling returns an immutable expansion request or a typed missing-input demand.
Execution occurs externally. Reply admission validates the exact request and
grammar, builds the actual generated inventory, and commits shared-budget
accounting before installing prepared state. The budget must match the run,
graph and epoch. Malformed/foreign replies preserve the original request and
charge no output. Completion returns actual records and an unsealed inventory.

The isolated macOS candidate passes 49 targeted controls. These include the
unchanged rustc nested-statement fixture, cold impl-producing nested macros,
unfinished children, sibling publication, block-local visibility, foreign/wrong/
duplicate replies, malformed output and foreign run budgets. The fixture harness
uses LRR2024; original upstream bytes, directives and licenses stay unchanged.
These observations are component evidence, not whole rustc fixture passes.

The asynchronous macOS integration gate passes with 1733 host/doc tests and all
49 targeted controls. Formatting, strict runtime/host Clippy, architecture and
first/final manifest audits pass. See
[the frozen checkpoint](../artifacts/macos-first/m014-recursive-adoption-checkpoint.json). The
upstream subset retains 46 component assertions, with zero full fixture/JIT passes.

M014 remains in progress. `SourceDiscovery` still needs to attach these fragments
to its nested declaration jobs, coordinate namespace/annotation inputs and cover
the complete graph and native facades before sealing. Intrinsic argument summaries,
mixed-edition generated calls, reached symbolic selection, full MX029/MX030, guest
JIT, release and Linux acceptance remain pending.

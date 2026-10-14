# M013 adopted child runtime macro contexts

M013 remains in progress. Standalone execution now supplies runtime macro contexts
for original and adopted source files with structurally present body macros. The
structural scan uses already captured syntax; it does not lower cold bodies,
execute user code, capture files, or execute a macro. Plain modules do not consume
the existing dispatch context slots. The contexts borrow owned sealed macro indexes
and the complete macro namespace assembled from actual source/module route facts.
The same route/prelude assembly is shared with discovery.

Each runtime index retains bounded snapshots of the actual inherited textual
scopes, declarations and cutoffs, plus their hygiene facts and expansion ancestry.
Entered-body lookup consults these frames after its own source scope. A later
parent definition cannot retroactively change a child's cutoff, and an inherited
user macro cannot silently fall back to the compiler macro. Snapshot sizes and
frame counts are checked before copying the outer scopes/declarations; aggregate
index storage is bounded to 64 MiB and runtime context count retains the existing
1024 limit. Publication/source identity checks remain in the dispatcher. Larger
macro-context scalability and complete quota boundary coverage remain pending.

Adopted providers now retain the prepared compiler facet. Early exported headers
carry a pending body owner until their actual module catalogue is available.
Import facts can be resolved while original-source execution requests remain
typed pending; a known header alone does not authorize expansion. Their real candidate
macro routes participate during staging, before import completion. Disclosed
child producers cannot change exports behind a validated cross-unit module alias;
unclassified producers and local export uncertainty remain pending. This lets a
real macro-only SDK import complete without prematurely closing the child source
or macro producer. The prepared compiler inventory and identities remain unchanged.

Ten new controls cover actual child assertions and aliases, scoped prelude policy,
real panic reporting, original/generated parent shadows, later parent cutoffs,
macro-only import discovery, and 1040 plain modules. Four successful CLI runtime
cases compare actual values with explicitly compiled isolated Rust1.99 macOS native
oracle copies. The panic/refusal and plain-module controls exercise actual guest
outcomes; they do not claim native comparisons. The original reached-phase failure
and alias-discovery diagnostic are preserved in the before/probe logs.

Native preparation probes also establish the pinned policy: bare assert! is a
compiler global under no_implicit_prelude; assert_eq! requires the SDK prelude.
An explicit root core alias is referenced from the child with crate::core.
The native failures for the initially assumed bare use core::assert path are retained.
These corrected test assumptions are not specification errata.

User body macro expansion, child eager compiler macros, nested adopted physical/
included source contexts, service coordination, full namespace/runtime/type/cache/
quota coverage, complete MX027/MX028 and JIT/release acceptance remain pending.
Linux remains a later port.

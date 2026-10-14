# M013 inline module macro-use inheritance

M013 remains in progress. Bare macro_use on an active inline module is recognized
as a compiler-owned structural attribute. The lexical index records a validated
module-to-parent export edge at the module's closing token. Candidates read the
child's local declarations and its own export edges; they do not follow the child
back to its parent and fabricate inherited exports. Ordered iterative traversal
preserves nearest/latest candidate order, limits depth/work/metadata, and preserves
pending child producers as typed input demand. Wrong-parent and duplicate edges
are refused before mutation.

Original and generated indexes use these same lexical edges. Generated outward
bindings enumerate the root's locally visible/exported declarations rather than
filtering only declarations whose original owner is the root. Canonical definition
identity, hygiene and origin metadata remain attached. Recursive child publication
therefore retains module-exported definitions for later generated and original
calls without leaking ordinary unmarked module declarations. Strict revalidation
still precedes a seal.

Six original/generated VM controls exercise exports, nested and recursive modules,
and source-order shadowing. A native probe confirmed a separate ambiguous case:
a generated macro-use export conflicts with an older original parent macro. That
case refuses publication with ambiguity and matches E0659. An unknown module
producer keeps the parent catalog incomplete despite independent progress. A scope
control checks pending/retry order and atomic wrong-parent/duplicate refusal.
All nine controls are actual acceptance tests, not proposed checks.

The initial failures and first partial integration failures are retained. The
boundary repair converts the exclusive module range end to its closing token, so
a following invocation without whitespace sees the export. This is an
implementation repair, not a pinned-backend contradiction. The native matrix now
has thirty successful VM/native comparisons, eight E0659 cases and the earlier
missing parent macro rejection. Native copies are isolated explicit oracle
preparation; no selected live runtime unit is compiled or loaded.

External captured-module and extern-crate macro-use forms, selective/list forms,
complete cross-fragment scope linking, Cargo/world/definition-unit facets, complete
origin invalidation, generated-body runtime macros, return/generic aliases, full
quota/failure boundaries, complete MX027/MX028 fixture/JIT acceptance and release
remain mandatory. Unsupported macro-use forms retain a transformation frontier;
they are not silently ignored. M014–M016 follow Section 57 order. Evidence is
macOS ARM64 Rust 1.99.0; Linux remains a later port.

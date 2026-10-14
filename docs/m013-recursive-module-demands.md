# M013 module demands from nested macro expansions

M013 remains in progress. Nested generated fragments now admit known module
source frontiers and retain their actual declarations instead of refusing every
external-module output. Module demands use the same deterministic, duplicate-
checked and bounded metadata index as original generated fragments. Unknown
transform/declaration producers continue to refuse before record admission.

A module-emitting child catalogue remains parked in the existing fragment tree.
Parent lookup selects the exact catalogue owning the module's expansion identity,
checks unique frame ownership, follows its real parent frames with a checked depth
and allocation bound, and joins the original captured provider. The existing
hygiene/origin/ancestry resolver uses each ancestor's actual invocation scope and
position. This preserves declarations before each expansion and excludes later
shadows. Retained records and invocation-file bases feed the existing adopted
module view without filesystem lookup or macro execution inside a query.

Three controls exercise a module emitted two macro levels deep, exact retained
record/header identity and adoption-gated ancestor lookup; two sibling modules
with separate logical scopes, physical preparation directories and parent cutoffs;
and an unknown attribute on a recursive module output that preserves the exact
pending producer without admitting a record or inventing a module demand.
The recursive and sibling controls preserve incomplete discovery after adopting
all currently selected module captures. Original compiler/include/string and
macro catalogue suites remain part of the host gate.

This increment retains nested macro-produced module demands and their context.
It does not yet process the child files' own declarations/imports/macros, publish
derived source inputs, recursively discover modules declared inside those files,
handle included-file physical layout, close the frontier or execute the child in
LRR. Qualified paths, complete Cargo/world/facet, origins/quotas and runtime/type
integration remain required. Full MX027/MX028, JIT and release acceptance remain
unfulfilled. The historical positive module LRR/native witness stays pending with
comparison_passed=false. Existing native parent-context comparisons are narrower
component evidence. macOS ARM64 Rust1.99.0 is primary; Linux remains deferred.

# T040 / E-R5-012 — combine actual declaration repr hints

Baseline51b35054d37a0c74169150ddc7fe8363d93be7cb; LOCAL049 solo shared
reservation. Read WT06 and existing claims; root simple-interpreter instruction
requires direct checkout validation, overriding this track's older frozen-copy
text. Primary unchanged witness: tests/ui/structs-enums/align-struct.rs, whose
AlignMany has separate align4,align16,align8 attributes and expects16. V430
owning2015 capture fails at7578dispatches/22source calls while constructing
Option::None from the real core assert_failed failure path. It is not a special
assert operand or an original-source exclusion. The source repr reader takes
last align8, causing a wrong observable layout before the secondary None error.

Invariant: one source declaration has one repr/layout answer shared by mem
intrinsics and guest addresses, independent of attribute order and source/macro
origin. Pinned rustc_middle/src/ty/mod.rs1619-1626 combines packed by minimum,
and1650-1652 combines align by maximum. Extend existing run_layout::declared_repr
only at these aggregation assignments. It still consumes actual NominalSchema
header attributes; no second parser, expression evaluator or layout algorithm.
Source/hygiene/edition/schema identity comes from ordinary declarations and
TypeSchema/SchemaNames; generated declarations join the same source inventory.
ensure_layout_facts publishes current DeclarationFacts; runtime/cell_layout::
nominal_layout and the one runtime/layout.rs algorithm serve both intrinsic
calls and pointer addresses. Cleanup/continuations/nominal construction remain
on their existing routes. No source preparation moves into installed execution.

Remove last-wins repr assignments. Retain existing syntax validation/number
reading; add no static rejection, checking, cap, accounting, sandbox or backend.
Cold cost is the existing O(attribute token count) pass once per declaration;
no new work on ordinary dispatch, AST copy, global namespace scan or per-op
planning. No performance claim. Run unchanged main in all editions, retain
layout/pointer controls and compare actual source/header repr results across
original/user emit/wrapper origins when applicable. Record any later failed
assertion rather than changing expected values or swallowing its failure.

Separate M018 hold exposed by the same witness: closed library facade calls
currently start arguments with no declared hints and skip later hint preparation
(run_facade::start_facade_call / RunMachine CallAdvance facade exclusion). A
fieldless generic value passed to an intrinsic may therefore lack its known
parameter context. Repair must consume actual selected declaration signatures
through common argument Type services, never an AssertFailed/Option/assertion
special case or guessed default. This slice does not close that architecture
obligation just because the corrected successful program avoids the failure path.

Required closure evidence: exact source review against all eight recurring modes
(distinct from MA002/MA003/MA006), frozen asynchronous macOS Rust1.99.0 builds,
actual unchanged runtime captures/cleanup/directives, source/HEAD/binary guards.
Any passing cases are runtime contracts only; T040, M018, interpreter/JIT,
full-fixture/tooling/diagnostic and complete suite gates remain held.

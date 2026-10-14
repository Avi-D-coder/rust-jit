# T024 initial source interpreter acceptance

The initial source path uses the prescribed pinned parser, owned structural
index, existing memo database and verified LRIR. No VM or run operation accepts
an AST object. A dispatcher resolves the current PC, owns the installed recipe,
then executes it against physical frame cells. Installed PCs bypass queries.
Input suspension retains the machine and phase; terminal faults never replay it.

Source lowering now supports the root block, initialized single-name lets with
primitive/absent annotations, expression statements, literal recipes and original
single-name local reads. Stable pattern-node identities identify local storage.
A reached parent hint flows into its initializer before execution. Let commitment
moves the checked initializer into a fresh binding; scalar local reads copy by
policy. Statement values are discarded explicitly. Root completion creates unit
or transfers the tail value into its result slot and returns a ticket. Dispatcher
return cleanup retires the initial scalar/storage inventory and consumes the
result ticket after creating a separate printable summary.

The normative source `fn main() { let x: i64 = 40_i64; x; }` executes eight
recipes and eight completed operations: one Literal, one Unit, two SetHint,
one ScopeEnter, one BindLocal, one ReadLocal and one Discard. Eight exits complete;
return cleanup separately reports one retired scope. Actual trace evidence has
zero Signature/BodyStructure/Thunk queries for the following cold function with
an unresolved generic bound and invalid names. Outlining its header is structural
source preparation, not a semantic query.

Source-dependent tail witnesses return 40, 41 and 42 from changed source/annotation
forms. A shadow initializer reads the old executed binding. Unsupported selected
calls and exhausted initial dispatcher steps are explicit faults; terminal
machines cannot execute again. Local opcode verification rejects non-pattern or
absent binding identities. Subprocess tests use Cargo's actual lrr binary and
assert the structured value/counters; selected unsupported calls, attribute macros,
statement attributes and nonzero entry parameters fail, producing no success JSON.
No selected guest source is compiled by this execution path.

The initial CLI is `lrr run FILE.rs`, using the existing capability-rooted
standalone source loader and immutable publication. Its explicitly developmental
JSON schema is `lrr/v2/development-run`; summaries contain printable values,
actual counters and installed-cache metadata. Runtime handles and worker-local
TypeIds are not serialized or accepted as executable client values. Other CLI
commands remain fail-closed until their ordered tasks. Complete schema-2 service
results and CLI/Cargo command contracts remain later integration.

Component scope is original scalar execution under E-57-001. Original simple
local routing is provisional until the macro-aware ordinary lookup pipeline,
constant-pattern classification, symbolic types and generated body origins are
integrated. Nested blocks/items, general paths, attributes, destructuring,
closures, calls, objects, numeric operators and expanded-body suspension are not
claimed. The current source adapter emits Unsupported for pending entered forms;
attribute/entry guards prevent silently ignoring those requirements. Macro/native
execution authority is never granted by source reading or query evaluation.

T025 still supplies the complete numeric inventory and Gate02. T030–T034,
T045 and M014/M016/M017/M038/M043 must integrate general block/control/call/test,
cleanup, generated-source/hygiene/epoch and release fixtures. Resource sizes are
checked in the current stores/index/install path; consolidated worker metadata,
fuel/cancellation and watchdog accounting remain mandatory later work. General
Drop activation order is not proved by these scalar cleanup tests. Full release,
JIT/performance/security evidence and actual Linux acceptance remain pending.

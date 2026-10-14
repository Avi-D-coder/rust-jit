# M013: compiler calls generated inside adopted physical children

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

The invariant is that compiler calls emitted by a child source macro retain the
actual adopted physical provider, child invocation cutoff, inherited parent
lexical frames, and physical source base through nested generated catalogues.
Compiler polling and captured input admission remain explicit coordinator work.

Generated compiler scopes now select the staged child provider rather than the
original root provider. They retain the inherited context separately and borrow
it when preparing eager operands or explicitly expanding a selected declarative
operand. Existing catalogue ancestry still supplies every generated parent frame.
The source base comes from the retained expansion identity; the compiler request
never derives a physical directory from the logical generated name.

Four acceptance controls exercise an emitted include in child/mod.rs with a
competing root part.rs, a second nested expansion producing include/concat,
original parent operand shadowing with a later conflicting definition, and a
macro-generated parent operand shadow. Every case runs the actual CLI interpreter
and compares its value with an explicitly compiled isolated native Rust copy.
No selected live runtime unit is compiled. The pre-change failure is retained.

The focused controls pass. The checkpoint records observed milestone, lint,
formatting, architecture and manifest results after completion. These cases do
not prove nested physical module adoption, arbitrary included/generated context
combinations, complete quota boundaries, service/facets/cache/runtime/type
integration, full MX027/MX028, JIT mode matrix, release or Linux acceptance.

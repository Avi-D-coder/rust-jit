# M013 recursive concat and environment operand chains

M013 remains in progress. The existing eager worklist now follows adopted outputs
of each concat/environment operand until it reaches one literal expression. A
returned macro is selected using its pinned normalized hygiene contexts and the
retained captured/generated lexical position. Missing outputs remain explicit
source/tool demands. No query execution runs a macro, captures a file or reads an
ambient environment variable. All execution/adoption remains coordinated separately.

The expansion-side producer proof retains every exact key and output digest. Parent
materialization re-observes each adopted record, checks the environment, grammar,
returned invocation and input identity, and validates that only the final output
is literal. The final parent output keeps its original invocation/input key. Each
operand has an independent proof; a returned output cannot inject extra arguments.
Concat accepts its existing pinned literal types, including numeric literals;
environment arguments retain their stricter string requirement.

One work budget charges frames and producer traversals, including already adopted
outputs. Checked metadata covers retained frames, aggregate operand proofs, token
forests and normalized hygiene facts. The same shared budget also bounds whole
include-path chain replay and include-view reproduction. The existing worklist
acceptance now proves that already adopted children cannot bypass a one-step limit
and that the exact three-step budget produces the unchanged parent output.

Twelve registered controls comprise eleven new tests and the strengthened existing
worklist control. They cover user/compiler returned calls, generated scope,
independent operands, numeric concat literals, absent returned macros, argument
injection and numeric environment-name refusals, exact adoption dependencies and
changed environment seals. Five concat programs and one nested environment/concat
program compare actual successful values with isolated native macOS ARM64
Rust1.99.0 programs. Native compilation uses oracle copies and explicit compile-time
environment input; it never compiles a selected live runtime unit.

The original two concat regressions failed with typed compiler-input errors. Their
baseline log remains separate from final acceptance. This extends whole include
path chains to concat/environment operands; it does not establish procedural string
execution, complete compiler/SDK attribute inventory/import/dispatch, all cross-unit/
world/edition paths, multiple source/build-output roots, full Cargo/environment/facet
preparation, compiler-expression lowering, remaining module/origin/quota/runtime/type
integration, M014–M016, full MX027/MX028/JIT fixtures or release acceptance. Linux is
still deferred. The checkpoint records actual final gate and audit results.

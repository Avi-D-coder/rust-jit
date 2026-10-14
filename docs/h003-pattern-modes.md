# H003 reached-pattern mode checkpoint

PatternContext carries the reached pattern's source edition and its
DefaultBindingMode (Move, Ref, RefMut). It begins in Move. Implicit shared reference
peeling selects Ref; mutable peeling changes Move to RefMut and never upgrades
Ref. Copying the context for a child preserves its siblings' inherited mode.

In edition2024, an explicit reference pattern and explicit mut/ref/ref-mut binding
modifier require Move and otherwise produce LRR-E-EDITION-PATTERN. Plain bindings
retain the inherited mode. Older supported source editions preserve mut resetting
to Move and explicit reference behavior. Unknown editions fail explicitly.
Contexts and modes have separate stable encodings; edition and mode both affect
context identity. These operations perform no traversal, copies, leases or commit.

The actual source-backed scalar condition-binding path checks its reached
identifier modifier from Move before binding preparation. Scalar mut and shadowing
execute normally; a skipped ref clause stays cold. A reached ref binding is valid
under the edition rule but still refuses reference execution with the existing
unsupported diagnostic. The checker does not make references executable.

The E2408 native fixture records explicit, inherited plain, nested reference and
mutable inherited bindings. A second source compiles and runs in 2021 while the
same inherited-mut pattern fails compilation in 2024. Tests retain actual compiler
stdout/stderr and native-only records. They never compile a selected live unit.

Full non-consuming reference/aggregate pattern tests, transactional commit, reached
versus unvisited arm behavior and full E2408 off/auto/force execution remain pending
through T050/T051/M028. Generated token editions require M014/M016. Async prelude
identities and reserved feature inventory are still H003 work. No full LRR fixture,
release, forced JIT or Linux acceptance is claimed by this checkpoint.

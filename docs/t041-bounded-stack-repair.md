# T041 bounded host-stack repair — scoped integration

The existing2000-frame guest recursion test must complete on its unchanged256KiB host stack, using explicit guest frames. The failed integrated milestone, marker witness and first extraction are retained. No depth, stack, budget or assertion was weakened.

The repair moves the six place arms and nineteen remaining lowering arms into pure out-of-line helpers. Main entry retains exact body/PC/source-AST checks, guard obligations and entered-attribute refusal. Original arm bodies, guarded binary ordering, source/recipe/origin identities, operation tags and effect/authority boundaries remain unchanged. There is no new production unsafe.

The exact v2 regression passed and was independently collected. A first broader driver failed evidence capture and introduced two unstaged deterministic runtime-alias probe files; none of its missing results are accepted. Production-identical v3 stages those existing probe literals before freezing and uses reviewed fail-fast per-command logging. All12 scoped checks pass with independent collection:448 actual host test bodies, all56 T040+21 T041 controls exactly once and full67 source-run bodies, including the unchanged stack test. Formatting, strict all-target Clippy and architecture pass. All3665 inputs/40133765 bytes stay fixed after every command.

Read-only disassembly of the accepted debug binary measures initial_thunk6048 bytes versus failed MAIN129008; the largest extracted helper uses28496 bytes. These are individual prologues, not total active-stack or performance/release proof.

Only the tested syntax/thunks.rs bytes are integrated. Named fields, temporary receivers, new stock arithmetic harness and their opcodes are separate pending candidates. A fresh MAIN macOS milestone is required after this integration. T041 remains in progress; full stock, JIT, release and Linux acceptance remain incomplete. See artifacts/macos-first/t041-stack-repair-components-integration-v3.json and t041-stack-repair-scoped-v3/independent-followup-collection.json.

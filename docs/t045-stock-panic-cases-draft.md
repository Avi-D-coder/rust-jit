# T045 original panic edge cases

Task/invariant: ordinary guest panic performs bounded cleanup before terminal publication, preserves its original message and kind, and never enters unreachable later panic code. This unvalidated successor adds unchanged rustc explicit-panic.rs, doublepanic.rs and panic.rs from ae0fe511b15ab760d7e963b13389d1bc2ab42b18, with full original directives, Git blobs, SHA256 and both original licenses. Existing original rows are unchanged.

Acceptance uses each original error-pattern to check the actual message. Default panic retains the explicit kind; the cold second panic has no demanded thunk; failed assert retains the assertion kind and existing Assert opcode rather than the explicit Panic opcode. Normal polling and whole original main bodies supply execution. There is no new production change relative to V7.

The external subprocess harness admits all eight original cases in distinct processes and both authorized native reference editions. Each case independently validates clean JSON panic exit, the original message patterns, corresponding explicit/assertion kind/opcode, bounded cleaned scopes and retained source origin. Native panic-hook framing remains separately disclosed; no nonzero exit, timeout or crash counts as a guest panic. No compiletest/full fixture pass is claimed.

Expected491 library bodies/13 T045 controls;177cases/297 originals. This private candidate is untested and unintegrated. Full T045/user Drop/destructor panic/host panic/worker isolation/release/Linux remain incomplete. Guest JIT stays deferred until interpreter convergence.

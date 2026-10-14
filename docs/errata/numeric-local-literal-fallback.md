# Bare local literals retain later source constraints

This records an implementation regression under the existing user-authorized Rust compatibility policy, rather than a specification or architecture change.

The unchanged upstream `tests/ui/consts/qualif-indirect-mutation-pass.rs` contains `let mut x: (Option<String>, u32) = (None, 0); let mut a = 10; *(&mut a) = 11; x.1 = a;`. The later written tuple member constrains `a` to u32. The new initializer fallback incorrectly made its declaration a strict i32 context. Four existing expression-prefix ownership controls then failed at the original BindLocal; the frozen revision-5 milestone recorded 504 passing library bodies and four failures before later executables ran.

An unannotated bare local literal now keeps its unresolved representation for later source-derived constraints. Wildcard/discard/comparison fallback remains separate; an operator initializer uses the existing bounded numeric operand path. This does not implement general local-use inference or certify this const-qualification stock fixture. The four controls explicitly supply source-derived u32 context and leave the Option/String initializer and const qualification cold.

The repair passed the four existing controls, the candidate's 506 library bodies, nine numeric controls and the unchanged division/remainder-by-borrow programs in all four editions. Native compiletest error-pattern formatting remains unexecuted. Formatting and strict library Clippy passed on the same frozen private source. MAIN integration and its new milestone are tracked in `artifacts/macos-first/root-local-literal-regression-v1/integration.json`; the failed milestone remains preserved. Task and revision-5 release acceptance remain pending.

The repaired integrated successor cleared all four original controls, then the CLI gate stopped at two stale policy expectations (889 project bodies passed, two failed). The adapter now checks successful f64 fallback and an explicitly typed incompatible binding. The preserved unannotated witness `let x=3+4; let later:i64=x;` still fails with LRR-E-OWNERSHIP because operator fallback precedes later-use inference. This remains unsupported interpreter behavior, not Rust compatibility acceptance; changing the adapter does not mark this witness passed.

## Successor implementation checkpoint

The bounded direct-use numeric operator constraints and shared borrowed shift RHS now integrate the successor repair. The ordinary bare-literal policy remains separately constrained; general numeric inference is incomplete. Current evidence and remaining frontiers are recorded in numeric-direct-local-constraints.md and docs/implementation-state.md; earlier failures above retain their historical scope.

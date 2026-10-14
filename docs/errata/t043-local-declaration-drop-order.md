# Integrated E-T043-001 witness and component regression

The unchanged native original compiled and ran successfully on pinned macOS
Rust1.99.0. The corrected V2 LRR scalar-retirement regression passed once; its
exact source bytes are retained in the V3 integration. V3's three temporary-borrow
controls and67 source_run tests also passed. This is component/native-reference
evidence; complete original Drop/or-pattern/RefCell execution remains pending.
Earlier unvalidated description below is retained as historical context.

# E-T043-001: local drop order follows declarations

Section13.5 originally said that locals are stored in successful initialization
order and destroyed in its reverse. The unchanged official rustc test
`tests/ui/drop/or-pattern-drop-order.rs`, lines30–39, contradicts that rule:
its first oracle declares primary bindings x,y,z, initializes z,y,x with tags
1,2,3, then asserts drop tags1,2,3. That is reverse declaration order z,y,x and
forward initialization order. Reversing initialization would yield tags3,2,1.

The copied original is unmodified, with both rustc licenses and COPYRIGHT.
Upstream commit: ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
Source SHA256: 82e4748da991120a15909e14ec275fc6efa4eb16038f74ae380c2915e7d5a064.
The complete original compiled with rustc1.99.0 (b940084d7,2026-09-28), edition2024,
and ran on macOS ARM64 with compile/run exits0 and empty runtime stdout/stderr.
This is native reference evidence, not an LRR or full compiletest fixture pass.
The retained report is
`artifacts/macos-first/t043-or-pattern-drop-order-native-oracle-v1/report.json`
(SHA256 8e3cd4d01c1c65d0913b68491b5af1f2b34b26f8c23d0c3fcb20d0f0b428c9c6).

Correction: retain each scope's primary local bindings in declaration order and
clean live values in its reverse. Their current initialization state determines
whether a value is dropped; a later assignment or reinitialization does not move
the binding to another position. The first primary occurrence supplies an
or-pattern binding's position. Parameter, field, temporary and array/Vec order
contracts remain as specified in their respective rules. This changes no
architecture, source profile, trait policy or runtime capability.

LRR already iterates primary binding cells in reverse source order. The new
regression,
`rustc_deferred_local_cleanup_keeps_primary_declaration_order_after_reverse_initialization`,
reads the first original oracle's pattern ranges, initialization sequence and
expected range from its AST. Its closure body is wrapped only for structural
indexing outside the licensed directory. The harness declares the original
primary bindings and installs their written u32 tags as scalar values to observe
retirement at every fuel boundary. It does not execute the closure, or-pattern,
LogDrop destructor, RefCell log or complete original source in LRR. Those remain
T044/T050/T053 and later trait/facade integration obligations. The independent
reverse-initialization model must disagree with the original expected order.

The private regression has not been compiled or run yet. MAIN is unchanged and
held for the T042 integration milestone; this erratum and regression are pending
review/validation/integration in Section57 order. No T043 completion is claimed.

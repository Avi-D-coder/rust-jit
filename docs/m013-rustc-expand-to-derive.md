# M013 official rustc expand-to-derive component

The unchanged upstream case is
`fixtures/upstream/rustc-checkout/tests/ui/proc-macro/expand-to-derive.rs`,
from rust-lang/rust commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
The licensed directory retains MIT OR Apache-2.0, both license texts,
COPYRIGHT, its auxiliary procedural macro and expected stdout. Its manifest
records the original Git blob, SHA-256, directives and file sizes.

The LRR harness lives in `xtask/tests/generated_derives.rs`, outside the copied
directory. It lexes the original file with its Rust 2015 source identity,
selects the actual macro definition and invocation token groups, and expands
them through the pinned declarative macro adapter. It does not rewrite the
case or replace its rule with an identity macro.

Acceptance `M013-RUSTC-EXPAND-TO-DERIVE` verifies the generated `Print` derive
occurrence and the actual input materialization: disabled outer and nested
fields disappear, enabled fields and `Inner` remain, nested `cfg_attr`
becomes `rustc_dummy`, the derive annotation is removed, and original span,
context and Rust 2015 edition records survive.

This exercises M013's generated derive payload invariant. It is a component
test only. The upstream `Print` procedural macro, stdout compiletest comparison,
whole-program LRR execution and full fixture acceptance remain pending.
LRR-specific authority and publication regressions remain necessary alongside
upstream language cases. M013 remains in progress; Linux acceptance is pending.

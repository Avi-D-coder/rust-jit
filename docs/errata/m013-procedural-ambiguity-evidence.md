# M013 procedural ambiguity evidence correction

This corrects a fixture and its evidence description; it changes no revision-4
requirement, architecture, task order or acceptance scope.

The earlier source worklist conflict fixture imported
`lrr_fixture_macros::emit_items`. In the pinned fixture source, `emit_items` is the
Rust implementation function for `#[proc_macro_derive(EmitItems)]`. The SDK export
is `EmitItems`, not `emit_items`; there is no function-like export with the latter
name. The actual earlier report returned LRR-E-CATALOG-INCOMPLETE and refused
publication after one native invocation. It proves unresolved-import refusal;
it does not prove two resolved macro exports became ambiguous.

The minimal witness is preserved in
[the earlier refusal report](../../artifacts/macos-first/m009-proc-session-99394-1791485295378739000/procedural-source-worklist-ambiguity/refusal.json)
and its sibling source.rs. The
[pinned macro fixture](../../fixtures/macros/session-workspace/src/lib.rs) declares
the derive export and the function-like `identity` and `session_counter` exports.
Historical reports and hashes remain unchanged.

The regression replaces the missing import with the real function-like
`session_counter` export. Both root-source and physical-module controls now require
LRR-E-MACRO-AMBIGUOUS specifically, rather than accepting generic catalog
incompleteness. Their actual server invocations, final refusal and source hashes
are recorded in the new module-procedural checkpoint. The broader original fixture
requirements remain pending until their full LRR acceptance runs.

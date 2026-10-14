# M013: reproducible matched macro-host server

This component repairs E-M013-001's native stdout/protocol collision on macOS
ARM64 with Rust 1.99.0. M013 remains in progress. This document describes the
initial prepared-server component checkpoint. Subsequent typed explicit
preparation and service/API MacroRun adoption are verified in
[the adoption checkpoint](m013-server-adoption.md). Full original-fixture and
CLI acceptance remain pending.

`python3 tools/prepare_macro_server.py --allow-tool-preparation` audits the exact
pinned rust-analyzer source and patches, requires the installed matching
rustc-dev component, and builds a separate external host package with an isolated
locked dependency graph. RUSTC_BOOTSTRAP is confined to that compiler-private
macro-host tool. The entry adaptation is an exact versioned patch over the
pinned CLI. The original CLI and protocol main loop remain unchanged.

Before expansion, the entry point duplicates the response descriptor and routes
process stdout to diagnostic stderr. The pinned main loop writes its API6
bidirectional postcard responses through the preserved descriptor. The client
continues to check the real executable hash, compiler identity, protocol, span
operations, output limits and separate macro authority. No guessed text is
filtered out of binary messages. The installed server is not overwritten.

Two versioned mechanical Rust1.99 server ABI patches implement the new
`injected_env_var` method. API6 supplies per-invocation process environment but
no compiler logical-env override. Returning None preserves the compiler client's
normal environment lookup; it does not fabricate a variable value. See the
exact compiler source witness and audited pre/postimages in the erratum and
vendor inventory.

Acceptance controls precede component acceptance:

- The unchanged copied `multiple-derives.rs` auxiliary executes First through
  Fifth once each against occurrence inputs from unchanged
  `derive-expand-order.rs`. Real outputs are empty. Five diagnostic prints are
  compared with the original stdout oracle and actual inputs as complete token
  shapes, including punctuation spacing. Token provenance and pretty-print
  whitespace are not a textual stdout compatibility claim.
- Preparation without the explicit authority flag fails before vendor audit,
  compiler capture or build. Its receipt is unchanged.
- The newly copied rustc `env.rs` auxiliary exercises the compiler's tracked
  environment API with the original THE_CONST=1 directive, absent variables,
  and both variables supplied. Only the unchanged unstable-feature macro host
  is prepared with RUSTC_BOOTSTRAP. Its real output declarations are inspected.

The original installed-server corruption regression remains required and is a
terminal-refusal observation. The positive prepared-server control is distinct.
Neither is a full compiletest pass, sealed source-discovery pass, guest execution
or JIT acceptance. Typed prepared-tool provenance and explicit preparation now
permit service/API adoption. Full fixture and CLI oracles remain pending.
Linux is unvalidated.

The preparation receipt records compiler/dev component, original entry,
versioned patch, dependency locks, source and real executable hashes. The
checkpoint records actual validation outcomes; planned checks are not passes.

The integrated macOS checkpoint has 1674 host/doc passes, all 721 M013 controls,
and 33 component assertions across 13 imported rustc cases. All 32 imported file
hashes are verified. Gate00/01/02 and provisional Gate03 component, formatting,
strict Clippy for both runtime and external tool, and architecture/manifest
audits pass. These counts retain zero full compiletest or full LRR case passes
and zero guest JIT acceptance. One unchanged empty-derive case retains the
separately recorded restricted interpreter CLI acceptance.

See [the checkpoint](../artifacts/macos-first/m013-prepared-server-checkpoint.json),
[the explicit preparation receipt](../artifacts/macos-first/m013-prepared-server-preparation.json)
and [the stdout erratum](errata/m013-procedural-stdout.md).

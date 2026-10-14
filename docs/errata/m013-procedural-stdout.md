# E-M013-001: matched binary macro protocol shares native stdout

Status: prepared-server component repair and service/API adoption verified;
unchanged original-source derive worklist now verified; full guest/tier and CLI
acceptance remain pending. The installed-server failure is
preserved. No backend or protocol is substituted.

The unchanged rustc `derive-expand-order.rs` and its unchanged
`auxiliary/multiple-derives.rs` at upstream commit
ae0fe511b15ab760d7e963b13389d1bc2ab42b18 expose a mismatch with the matched Rust
1.99.0 API6 external server on macOS ARM64. The auxiliary's `First` derive prints
its input with `println!` and returns an empty token stream. With the mandatory
bidirectional postcard transport, those human-readable bytes appear on the same
stdout pipe as binary responses. The client returns `LRR-E-MACRO-CRASH` with
`failed to decode message: The original data was not well encoded`.

Minimal witness: one invocation of `First` on the original `Foo` item after its
first derive group is removed. Captured protocol stdout includes `Derive First:`
and the later Second, Third/Fourth and Fifth groups between binary records.
The original five-occurrence positive probe fails on the first invocation; its source
and failure logs are retained under `artifacts/macos-first/m013-source-derive-input*`.
No guest runtime unit was compiled, and the original auxiliary was not silenced,
rewritten or replaced. Only that macro-host source was explicitly prepared.

The pinned server CLI's main function passes process stdout directly to its
message loop. The postcard branch emits binary responses to that same writer;
the expansion branch calls the loaded macro in that process. See
`vendor/rust-analyzer-macros/crates/proc-macro-srv-cli/src/main.rs` and
`src/main_loop.rs`. Those pinned sources remain unchanged.

The regression records this as failed procedural fixture execution, requires the
specific decode diagnostic, observes one native attempt and terminated helper,
and checks that retrying the same or next occurrence performs no additional
native invocation. Its success certifies terminal refusal only. It cannot count
as a passed stock Rust case, successful expansion or discovery seal.

Correction requirement: preserve the exact matched server/protocol, span and
authority contracts while separating native macro diagnostics from response
framing. Explicit preparation now builds a versioned external entry adaptation
with a duplicate protocol descriptor and native stdout routed to diagnostic
stderr. The unchanged auxiliary successfully executes all five derives on
original-source occurrence inputs. Actual empty outputs and printed token shapes
match the unchanged oracle. This is a procedural component control; complete
original-fixture orchestration and exact diagnostic/CLI oracles remain required. Do not relax
framing validation, filter guessed text from binary messages, switch transport,
fabricate empty output or exempt printing macros to obtain acceptance.

Rust1.99.0 rebuild witness: the pinned server's two span implementations lack
`server::Server::injected_env_var`. The isolated build failure is retained in
`artifacts/macos-first/m013-prepared-server-build-second.log`. API6 exposes the
per-invocation process environment but no separate compiler `--env-set` overlay.
The versioned mechanical compatibility patches return None for that absent
overlay, preserving the client's normal environment lookup and existing tracking.
They add no environment access or provider effect. The original source inventory
and patch pre/postimage hashes remain independently audited. The prepared server
entry point and build provenance are separate from the installed sysroot server;
typed service/API adoption now retains the original component identity and adds
registered repair-source, adapter, build-receipt and real capability provenance.
Full fixture and CLI acceptance remain pending.

The installed exact rustc-dev sources confirm this fallback: `proc_macro/src/lib.rs`
uses `BridgeMethods::injected_env_var(key).map_or_else(|| env::var(key), Ok)`;
`rustc_expand/src/proc_macro_server.rs` sources injected values from the separate
compiler `logical_env` map. This map is not represented by the pinned API6 task.
The prepared host entry point's Darwin link search/rpath is captured from the
same compiler's sysroot, rather than selecting another LLVM or compiler library.

The unchanged copied rustc `env.rs` auxiliary also verifies the ABI fallback with
THE_CONST=1 and missing ANOTHER, both missing variables, and both supplied. The
three real output declaration pairs are checked. Its unstable-feature macro-host
build uses explicitly scoped RUSTC_BOOTSTRAP; no selected guest runtime unit is
compiled. The adapter and runtime dependency closures do not gain rustc-private
libraries. See [the preparation and acceptance scope](../m013-prepared-server.md).

Section48.6 now records the narrow registered `stdio-v1` exception. An
outside-sysroot path without its typed provenance remains invalid. Explicit
preparation recaptures the original tuple and builds the audited external tool;
MacroRun performs no build. The checked variant has an actual service/API
publication control and a changed-source/no-run refusal control. See
[the adoption scope](../m013-server-adoption.md). The original installed-server
failure remains required and is not relabelled as a passed fixture.

The unchanged source now executes all five native derives through the owning
MacroRun/discovery worklist and publishes Foo with its canonical source identity.
The auxiliary's macro_rules-generated registration headers are expanded on demand
through the pinned engine and owned syntax bridge, then matched against the exact
loaded export set. Its 2015 definition site retains a captured host source while
the call site remains 2024. Complete printed token payloads match the unchanged
oracle. Guest main execution, exact textual stdout, CLI and full tier acceptance
remain pending. See [the worklist scope](../m013-original-printing-worklist.md).

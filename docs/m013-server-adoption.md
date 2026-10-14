# M013: prepared-server adoption

E-M013-001's `stdio-v1` repair now has a typed explicit preparation API in
`project::prepared_macro_server`. It checks build and macro grants before
filesystem/process access. It recaptures the original Rust1.99 compiler,
sysroot and component tuple before preparing the registered external server.
The exact source/patch/lock inventory is checked against the runtime's registered
hashes; the pinned preparation script audits the complete vendor closure and
builds only the external macro-host package. No selected live runtime unit is
compiled. SDK installation and downloads are absent.

The same explicit preparation performs real attribute token/span echo and
function-like identity probes with separately authorized, exactly hashed host
artifacts. It captures the original component server, compiler-dev manifest,
source pin, adapter/source/lock/host build identity, repair files, build receipt,
actual server content and capability outcomes. Source and artifact contents are
compared again after the probe. Source retention and each read have checked bounds.

Preparation schema4 retains the original strict sysroot requirement when the
optional typed provenance is absent. Its registered variant binds the executable
to the exact sources, compiler, adapter and API6 capability outcomes. Removing
the provenance, changing the pin, omitting a source or supplying a different
compiler tuple is refused. Unknown record fields are rejected. The toolchain
stable-encoding domain advances; old publication keys require explicit
repreparation rather than relabelling cached records.

`PreparedFiles` captures every additional recorded file. Existing transaction,
MacroRun adoption and publication checks therefore also cover the repair source,
baseline server, compiler-dev manifest, compatibility artifact and build receipt.
Live MacroRun never builds the server; its separate macro authority and pure
provider/coordinator guard remain intact.

The positive control publishes and reloads a schema4 preparation, opens its
prepared server through the ordinary MacroRun, executes identity once and a
nested declarative macro once, discovers generated `made`/`alias` members, and
publishes the real discovery seal. Foreign/empty run publication and pure query
execution remain refused by the shared acceptance driver. The edit control
copies the exact source/binary bundle into an isolated fixture, verifies it,
changes only that fixture's entry source, and requires stale refusal before a
MacroRun can be created. No guest macro invocation or seal is fabricated.

This is service/API component integration. Original five-printing-derive and
tracked-environment controls remain separate upstream evidence. Complete original
source-fixture orchestration, exact stdout/diagnostic oracles, CLI preparation,
remaining M013 matrices, later traits/runtime/JIT tasks and Linux acceptance
remain pending. M013 and the full project are not complete.

The integrated macOS checkpoint records 1676 host/doc passes and all 723 M013
controls. Gate00/01/02 and provisional Gate03 component, formatting, strict Clippy
for runtime and host tool, architecture and manifest audits pass. The 13 imported
rustc cases retain 33 component assertions and 32 verified unchanged files.
Full compiletest/LRR cases and guest JIT acceptance remain zero; the separate
restricted interpreter empty-derive case is retained.

See [the checkpoint](../artifacts/macos-first/m013-server-adoption-checkpoint.json).

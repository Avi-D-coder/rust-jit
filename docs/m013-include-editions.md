# M013 captured dependency include editions

M013 remains in progress. Eager compile-input worklists and nested macro selection
validate supported source profiles independently of live entry selection. Exact
root/nested token unit, content and effective edition checks remain mandatory;
incompatible semantics and mismatched caller editions fail before output or capture.
The selected live entry remains Rust 2024.

Pure included-item parsing retains the supplied owning source edition and lexical
module/cfg context through the pinned parser. Unknown editions, incompatible
semantics, wrong units/kinds, malformed syntax and mismatched capture identities
remain typed failures. This parser consumes an adopted immutable capture record and
has no filesystem or external macro execution authority.

Four new controls exercise 2015/2018/2021 item/expression/nested includes through the
actual eager source driver, including a `gen` function accepted in older editions.
Each edition has native owning-edition oracle programs for item and expression
scope, nested base paths and malformed syntax. Output spans retain the source
edition, content, unit, context and origin. Parent capture/output adoption, forged
output refusal, incomplete nested dependencies, removal suspension and retained
snapshot checks run in those edition cases. Item outline controls retain cold bodies,
apply cfg and preserve origin-bearing parse failures. Profile refusal checks run
before capture adoption. Tests compile only explicit isolated native oracle copies.

This is include transport/parser component evidence. End-to-end include scheduling
inside SourceDiscovery, complete dependency/world facets, mixed-origin generated
edition semantics, origin/invalidation/quota/runtime/type integration, full MX027/
MX028/JIT fixtures, M014–M016 and release acceptance remain pending. Linux is deferred;
Docker is supplementary. No complete M013 or release claim is made.

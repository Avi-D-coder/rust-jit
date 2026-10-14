# M013 explicit module candidate capture

M013 remains in progress. Pure module layout now separates candidate spellings from
selection, sharing the existing ordinary module path rules. It preserves raw path
components so capability resolution processes symlinks before parent components.
The source authority routes one explicit path or both default candidates into a
frozen request tied to the authority generation, declaration producer and origins.
This is a module source request, not certification of a compiler macro by spelling.

The explicit SourceCapture service observes each candidate under its existing
capabilities or immutable build-output tree. Only NotFound or recorded output absence
becomes an absent observation. Existing non-file nodes retain separate presence
identity; they cannot silently count as missing. Captured directories participate in
build-output membership, including empty directories. A selected non-file refuses
loading; two present candidates report ambiguity. Captured regular files retain the
same bounded byte reads, virtual/physical locations and revalidation as include data.

A CapturedModule contains immutable bytes/presence/absence evidence and a stable
request/output identity. Capture re-observes all candidates before returning and
checks source roots; revalidation detects changed bytes, physical alias, non-file
identity or candidate membership. Requests and retained records have checked metadata
bounds, with at most two candidate files under existing per-file byte limits.
Returning the record grants no query publication or frontend completion authority.
Existing include failures retain their typed diagnostics.

Eight controls exercise candidate selection, immutable bytes, missing/new candidates,
ambiguity, non-file presence, symlink-parent order and escape refusal, changed
bytes/authority, captured build-output files/empty directories after disk removal,
resource refusal and a controlled membership race before record return. Four actual
macOS ARM64 Rust1.99.0 programs compare captured source bytes and original file
origins for plain files, mod.rs, path attributes and symlink-parent paths. The existing
include-capture suite also passes. A separate native directory-candidate probe reports
E0761, supporting presence rather than interpreting a non-file as absent.

This is the coordinator capture part of generated-module integration. Adoption into
immutable query inputs, matching a retained generated declaration demand, entering
the child scope and parent macro context, scheduling child declarations/imports/macros
and actual LRR child-function execution remain mandatory. The historical native
positive module witness remains status=pending/comparison_passed=false; capture
controls do not turn it into a passing full LRR fixture.

Complete origins/invalidation/quotas, compiler/SDK attributes, procedural strings,
Cargo/world/facets, runtime/type/compiler-expression integration, M014–M016, full
MX027/MX028/JIT and release remain required. Linux remains deferred.

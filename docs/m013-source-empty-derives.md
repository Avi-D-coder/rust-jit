# M013 original-source empty derive discovery

The unchanged rustc `deriving-meta-empty-trait-list.rs` case now exercises direct
captured-source discovery, rather than only a generated-output annotation plan.
Its original item starts hidden behind an attribute frontier. A pure private
certificate resolves every non-inert outer annotation through the source macro
namespace and exact compiler attribute catalog, parses each derive list through
the pinned adapter and verifies all enabled member attributes. Only empty lists
with settled outer/member attributes receive a certificate.

The certificate binds the captured outline, path, header, exact compiler
definitions, original-source input identity and complete preprocessing facts.
It contributes to seal membership; its semantic dependencies are the captured
source outlines and selected namespaces, not an invented external input or macro
output. Selection replays before sealing. The worklist restores only the certified
original declaration and its real constructor/type routes, retires its exact
frontier and completes its ordinary textual producer. It creates no expansion
record and invokes no native macro or compiler.

Controls cover the original copied rustc input, repeated/cfg_attr annotations,
struct/enum/union members, disabled unknown members, compiler aliases, nonempty
groups, unknown outer/field/qualified attributes and noncompiler definitions.
Database publication preserves exact dependency equality. A changed captured
source suspends the old seal, including a change confined to a cold body.
The first discovery failure and a later publication bookkeeping failure are
retained as implementation witnesses; neither is a pinned-backend contradiction.

The initial integrated gate hit the unchanged cumulative macro deadline in nine
native controls, leaving later tests unexecuted. That failure is retained.
Further validation explicitly uses `RUST_TEST_THREADS=2` to reduce host contention;
production deadlines and the required test inventory remain unchanged.

This is component discovery/publication evidence. Full native rustc/compiletest
and LRR case acceptance remain pending. Nonempty original-source procedural or
builtin derives, interleaving empty groups with replacing attributes, legacy
helper ordering and complete transformation/import/resource matrices remain
required. M013, release and Linux acceptance are incomplete.

See [the checkpoint](../artifacts/macos-first/m013-source-empty-checkpoint.json),
[the prior generated-module controls](m013-helper-modules.md) and
[licensed upstream fixtures](../fixtures/upstream/rustc-checkout/README.md).

# M013 compiler-only empty derive progress

The unchanged rustc case
`tests/ui/derives/deriving-meta-empty-trait-list.rs` is copied at commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` under the original MIT OR Apache-2.0
licenses. Its discovery adaptation lives outside the copied directory. The
pre-fix control stayed incomplete after a real declarative expansion when no
procedural catalog was present; `m013-empty-derive-before.log` records this
implementation gap, not a specification contradiction.

The catalog now consumes only the next positively resolved compiler derive
annotation with an empty trait list. Private certificates bind the original
output/header/cfg/epoch, annotation index and exact compiler definition. Enabled
member attributes must be known inert attributes before compiler-only settlement;
unknown members retain the producer. Member proof fingerprints use explicit
stable encoding. Certificate counts, retained metadata and temporary work vectors
are checked and bounded. Selection and member proofs replay before sealing.

No native request, helper declaration or synthetic expansion record represents an
empty derive. Pure progress can publish a certified original item or advance to
the next transformation. Native helper decisions no longer vacuously consume
empty groups; each empty group needs its own certificate. Real derives retain
later empty groups in their input and keep the original producer open until those
groups are consumed.

Controls cover the original `_Bar` case without any procedural catalog, repeated
empty groups, known inert members, disabled unknown members, unknown outer/member
and qualified annotations, a following replacing attribute, and empty groups
before/after a real native group. The following-attribute control records three
native calls, zero native derives/helper declarations, the replacement and
original-item absence. The surrounding-empty control records four native calls,
two actual derive outputs and original/constant retention. Both seal and publish
through the owning run; their reports explicitly reject full fixture acceptance.

Ten copied cases retain all original helpers, directives, snapshots, licenses,
COPYRIGHT and recorded hashes. Discovery of this case is component coverage,
not its complete compiletest or LRR execution. Full copied-fixture acceptance,
original-source/builtin integration, wider transformation/namespace/nested/import
matrices, legacy helpers, remaining M013 requirements, later milestones,
release and Linux acceptance remain pending. M013 remains in progress.

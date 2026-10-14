# M010 declared editions for macro-host source facets

Source helper declarations and catalogs now select the producer unit's recorded
edition. The crate-private macro-facet parser uses the same pinned structural
frontend in 2015/2018/2021/2024 mode. Its parse key and declaration fingerprint
retain that edition. MacroSession invokes it only after exact ProcMacroHost,
loaded-producer, frozen preparation and captured source-root admission.
The public live-root parser still calls validate_live and rejects older selected
roots. No body inference, source compiler invocation or different frontend is added.

The private parser regression reads an unescaped gen export/helper in three old
editions, distinguishes their parse identities, rejects the same declaration in
2024, and confirms old live profiles and unknown editions refuse. The real
preparation fixture separately compiles one proc library in each old edition and
keeps its consumer unit in 2024. Complete source catalog admission supplies gen
helper metadata and an identity-bound binding. A false 2024 definition edition
refuses. Native Rust 1.99.0 accepts each library in a 2024 source with raw derive
and helper syntax. Session metadata operations execute zero native invocations.
Each run retains actual Cargo/SDK/artifact preparation and native diagnostics.

This proves original-source helper metadata and native membership for old macro
host units. Full generated-token edition propagation and mixed-edition guest
execution remain later M037/integration obligations. Generated catalog expansion,
source membership, full MX018–MX020, release and Linux acceptance remain pending.

# E-M015-009: procedural Test candidates use both catalog startup paths

M015, revision 4 sections 49/50.4: procedural attributes and compiler Test
registration are distinct facets even when their source spelling is `test`.
Metadata installation grants no native preparation or macro execution authority.

The unchanged official ambiguous-builtin-attrs-test.rs and builtin-attrs.rs at
rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18 provide a procedural `test`
attribute returning `struct Test;`. A second adaptation imports the unchanged
auxiliary/test-macros.rs identity_attr as test; its returned function retains no
compiler Test marker. Both must execute their selected procedural Attribute
exactly once through MacroRun, publish that real generated definition, and seal
an empty test catalog without preparing a signature or body.

The strict early-path witness used the public with_declarative_procedural(...)
constructor followed by with_compiler(...). Preparation and export catalogs
contained the exact expected Attribute exports. Discovery nevertheless sealed
one original function as a compiler test, with zero callbacks, zero expansion
records and no selected procedural demand. Both strict callback assertions failed.
Actual publication/catalog/source identities are preserved in the V2 failure
archive. No expected output, expansion record or macro invocation was invented.

The separate legitimate with_declarative_compiler(...).with_procedural_catalog(...)
path passed both native controls: one callback/record each, actual named Struct
Test or renamed identity function, and repeated empty sealed catalogs. This
isolates configuration lifecycle rather than macro output or token hygiene.

Correction: both public installation paths call one bounded original-Test
candidate reservation helper. The early path reserves after captured provider
configuration, before discovery begins. The helper preserves existing producer,
frontier, namespace hiding and source-position identities from the late path.
Only actual facet selection creates a procedural demand; its fulfillment still
requires the explicit macro authority, immutable preparation and matched server.
No selected live runtime unit is compiled, and no arbitrary symbol is loaded.

Private unchanged strict early/late/non-function joint acceptance:
artifacts/macos-first/m015-required-admission-repair-proc-test-shadow-nonfunction-joint-v1-acceptance.json
(eight controls, six actual native calls), plus the existing catalog/procedural
controls in m015-required-admission-repair-proc-test-shadow-catalog-joint-v1-acceptance.json.
The independent worker early/late and module/constant controls passed against the
exact copied repair. The suspected competing original/nested jobs on non-function
procedural targets did not reproduce; existing replacement proofs retire them.
No production change was made for that hypothesis. The historical zero-selected
source-only probe is retained as unexecuted and does not count as acceptance.
Fresh integrated macOS milestone passed 2022 host/doc checks against 3501 unchanged source/include inputs and membership, all nine checks zero: artifacts/macos-first/m015-procedural-test-startup-main-acceptance.json. The official source/output closure and original
licenses remain unchanged; adaptations stay outside the licensed directory.
This is component evidence, not complete stock fixture parity. M015/full
MX031/MX032, guest JIT, release and Linux acceptance remain pending.

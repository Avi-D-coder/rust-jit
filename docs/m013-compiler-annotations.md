# M013 compiler annotation facets in production discovery

Revision 4 Sections 49–51 and Section 57 milestone C, macOS ARM64 and Rust 1.99.0.
M013 remains in progress. The invariant is that compiler annotations are selected
by exact definition identity through captured namespaces. Selecting an annotation
cannot grant helper declarations, procedural execution, producer completeness or
publication.

The standard CompileInputCatalog now contains the pinned compiler-owned derive
and test attribute facet. Its compiler globals and edition-prelude routes expose
the same core-owned definitions; std reexports retain those identities. Derive is
also exposed at core/std roots. Test is exposed through compiler globals and the
preludes, with explicit absence at crate roots. The export distinction was checked
against library/core/src/lib.rs, library/core/src/macros/mod.rs and the core/std
prelude sources in the user's official checkout. Custom compile-input facets
retain no standard annotation catalog. Forged definition provenance refuses.

Existing SourceDiscovery route installation and GeneratedMacroCatalog captured
namespace construction consume this inventory without a separate resolver.
When a generated ADT is blocked by a derive annotation, production discovery now
uses GeneratedDeriveResolution and its bound plan to suspend on the first exact
ordered derive occurrence. That occurrence binds the actual output, selected
annotation, header, cfg and expansion epoch. This is still a demand: export/helper
selection and eventual appends are unfinished. Unknown annotations and helper
candidates cannot be guessed inert or discharged by this branch. Original-source
attribute planning, full generated helper classification and positive replay before
final publication remain required.

The unchanged rustc derive-expand-order case now uses the production standard
compiler facet for its five ordered occurrences, rather than a test-constructed
attribute inventory. The labelled identity-MBE envelope stays outside
[the licensed copied directory](../fixtures/upstream/rustc-checkout/README.md).
This remains a component assertion; full native/compiletest/LRR acceptance for
all seven imported cases stays pending.

The actual authorized chain_begin/chain_finish control attaches the compiler facet
and reconstructs the expected occurrence from the adopted native output. It checks
that production discovery blocks on that same occurrence, observes exactly two
native attribute invocations and two accounted outputs, and refuses sealing and
publication. No derive runs. The saved refusal includes the demand digest and
compiler-annotation-selection observation. Existing preparation, matched external
server, separate macro authority, exact input adoption and run ownership remain
in force. A test-placement error and an incorrect fixture item name were corrected;
the failed logs remain evidence.

Inventory tests check core/std identity across four source editions and each
edition-prelude path, exact provenance, compiler-global membership and root test
absence. The original API-missing acceptance compile and subsequent focused/native
checks are retained. Final host/gate/audit results belong to the checkpoint.

Next steps remain production source and generated derive export resolution, exact
helper declaration suspension/adoption, item-local helper precedence, ordered
execution/appends, positive replay and final publication. Complete world/edition/
Cargo/CLI/service fixtures, M014–M016, broader language/traits/JIT/native/security/
performance, release and Linux acceptance remain pending.

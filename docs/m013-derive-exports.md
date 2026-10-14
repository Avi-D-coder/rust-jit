# M013 ordered generated derive export selection

Revision 4 Sections 49–51 and Section 57 milestone C, macOS ARM64 with Rust
1.99.0. M013 stays in progress. The invariant is exact export identity and ordered
occurrences before helper declaration admission or procedural execution.

The pinned derive-path parser now returns each path's original token forest.
Generated plans retain owned forests, preserving actual spans, contexts, editions,
raw names and repeated occurrences. Runtime metadata and token retention remain
bounded. No display-string reparse or fabricated source spans are introduced.

GeneratedDeriveResolution composes annotation replay with a new pure export
selector over the existing captured macro namespace and PreparedProcCatalog.
Selection checks the consumer, environment and epoch, and preserves exact loaded
artifact/export/kind/definition provenance. Aliases keep the definition; repeated
paths have distinct occurrence and helper-demand identities. Absolute, crate, self,
raw names and dollar-crate paths use actual token edition and emission ownership.
Mixed contexts/editions retain a typed namespace demand. Noncatalog definitions,
absence, pending, ambiguity and inaccessibility remain explicit outcomes. A wrong
prepared procedural kind refuses rather than dispatching as a derive.

A selected export yields a demand bound to the occurrence, complete catalog and
exact definition. It grants no helper metadata, execution or completeness. Replay
refuses context/membership changes and positive choices that become absent,
pending, inaccessible, ambiguous or different definitions. Production
GeneratedMacroCatalog now uses these choices when blocked on an ADT derive and
suspends on the first ordered declaration demand. Exact helper declaration capture,
item-local precedence, execution/appends and final replay/publication remain pending.
Original-source derive integration and builtin derive payload selection also remain
required; the compiler annotation facet alone does not implement those payloads.

The unchanged official rustc derive-expand-order source exercises five ordered
choices in a labelled edition-2015 macro-template envelope and controlled pure
catalog. This is namespace/identity component evidence, not a loaded native export
or full compiletest pass. LRR-specific controlled-catalog tests cover aliases,
repetitions, missing/ambiguous/changed replay, wrong kind/context, and qualified paths.
The qualified-path variant uses edition 2024; the initial edition-2015 namespace
fixture did not provide an explicit extern-crate route for its absolute path.
[All seven copied cases retain their original licenses and provenance](../fixtures/upstream/rustc-checkout/README.md).
Full native/compiletest/LRR acceptance remains pending for every imported case.

The strengthened actual authorized chain control uses the matched loaded catalog
for CheckDeriveInput and Empty. It compares production's blocked demand to the
selection reconstructed from adopted native output, records distinct ordered
helper demands, and retains the exact output wire, expansion key and export choices.
It checks two native attribute invocations and two accounted outputs, with no derive
execution, sealing or publication. Separate preparation/execution authority,
matched external server, run ownership and input adoption checks remain in force.
The evidence field derive_occurrence_demand now records the selected occurrence's
helper-declaration demand; the choices file retains both identities explicitly.

Initial API-missing acceptance, fixture namespace/edition assumptions and private
wire/serialization API failures are preserved with the corrected passing evidence.
Five focused export controls, the existing path-fact checks and the actual native
suspension control pass. Final gate/audit counts belong to the checkpoint.

Next work is exact helper declaration suspension and source-facet adoption,
item-local helper precedence, ordered authorized execution/appends and final
publication. Complete transformation/world/edition/Cargo/CLI/service fixtures,
M014–M016, traits/language/JIT/native/security/performance, full fixture, release
and Linux acceptance remain mandatory.

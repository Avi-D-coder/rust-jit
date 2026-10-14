# M013 ordered derive append integration and verified publication

Revision 4 Sections 18.2, 49–51 and Section 57 milestone C, macOS ARM64,
Rust 1.99.0. M013 remains in progress. This component integrates retained derive
outputs through the existing child-discovery and namespace pipeline, and releases
the owning item only after its derive children have completed.

After execution, the generated catalog returns an explicit Append demand with the
exact reconstructed invocation and retained immutable record. Discovery retains
that demand separately from native execution. MacroRun integration requires the
coordinator, the shared run budget, the exact pending request/descriptor and an
output owned by that run. It verifies recorded inputs and the database's current
epoch and record. Forged descriptors and foreign runs cannot integrate the output.
The append phase performs no additional native invocation or build.

The retained output enters the existing generated child frame, header/import
planner, recursive producer scheduling and route snapshot machinery. Wire bytes
already retained by execution are not charged or admitted twice. The original
attribute producer remains open while children finish. Their outward bindings
contribute ordered prefixes at that same producer position; ordinary lookup stays
incomplete until explicit final completion. Wrong producer IDs and repeated final
completion refuse without removing that blocker.

A completed derive child merges through the same metadata integration routine as
ordinary generated children. Every occurrence remains separately recorded for
positive replay. Once all occurrences for an item are appended, the original
producer retires and a certificate-backed header plan publishes only the settled
original item routes. Helpers and original tokens remain unchanged. Nested output
producers and imports must complete through their normal pipeline before a child
can retire. Cross-output shadowing and the complete nested/hygiene/edition matrices
still need acceptance; this component does not certify those cases.

Helper declaration dependencies now accompany DiscoveryInput as typed exact
request/receipt pairs. Stable encoding includes both. Checked retention accounts
for request/receipt metadata. Publication validates matching receipts, strict order,
epoch, graph, parent descriptors and prepared derive export identities before
requiring exact equality with the seal's dependency list. The native acceptance
initially exposed this missing publication field; the check remains strict.

The actual matched-server chain executes CheckDeriveInput then Empty after two
replacing attributes, integrates the const and empty outputs, retains the original
Marker, and excludes the discarded function. It observes four native invocations,
refuses forged/foreign-run append admission and direct publication, then publishes
through the owning MacroRun after actual input verification. The DiscoverySeal
query returns the published proof. Its wires, keys and ordered output identities
are retained. The prefix regression checks incompleteness, ordering and wrong or
duplicate completion. Earlier pending controls explicitly defer append and remain
incomplete; they are not full fixture passes or skipped successes.

These LRR-specific authority and integration controls supplement the unchanged
licensed rustc cases. Eight cases, twenty-one verified original files, fifteen
component assertions over seven cases and all pending full native/compiletest/LRR
acceptances are unchanged. No whole rustc fixture is counted as passed here.

Next work includes cross-output and nested producer/import acceptance, later
replacing attributes, complete final positive replay matrices, original-source and
builtin derive payloads, pure/source parent provenance, generated macro-host
facets, complete scope/world/edition/Cargo/CLI/service coverage, later tasks,
full fixtures, release and Linux acceptance. This component does not complete M013.

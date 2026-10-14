# M013 bound helper declaration requests and worker receipts

Revision 4 Sections 49–51 and Section 57 milestone C, macOS ARM64 and Rust 1.99.0.
M013 remains in progress. Helper names must come from the exact captured macro-host
source declaration and loaded export. Metadata admission cannot invoke a derive,
discharge its producer or publish a catalog seal.

The pure export selector now creates private-field declaration requests bound to
ordered occurrence, demand, complete catalog, definition, consumer/environment/
epoch/preparation and parent expansion descriptor. Changed catalogs, unresolved
exports and invalid indexes refuse construction. Repeated occurrences keep distinct
requests even when they select one definition. Stable encoding uses explicit fields.

MacroRun resolves the request outside SourceDb providers. It checks macro authority,
frozen preparation/context, ownership of the actual adopted procedural parent and
the matching database record before source capture. A foreign run cannot borrow
another run's output from the database. The worker rechecks the matched session's
catalog/export, reads only the preparation-captured macro-host root with an 8 MiB
allocation bound, verifies its content digest, and parses source-facet headers with
the exact macro-host edition/cfg. It registers the selected original declaration
through the existing MacroSession source-facet checks, then verifies recorded inputs
and deadlines again before returning. No build or procedural invocation occurs.

A concrete original declaration supplies a receipt bound to the exact request and
source-declaration identity. An explicitly empty declaration is ready empty data;
unavailable API6 helper metadata is never treated as an empty list. Source producers
or missing declarations yield typed NeedExpansion/NeedSource outcomes. Those
pending branches are implemented but have no new acceptance witness in this
checkpoint. Generated declarations and complete macro-host facet integration stay
mandatory under E-M010-001; source spelling alone cannot supply metadata.

This worker currently requires a procedural parent adopted by its own run. Pure
macro/source parents still need their production provenance/adoption integration.
Requests can be created from pure plans, but that construction is not worker or
execution authority. Receipts remain pure facts: validate_request proves demand
identity only and does not revalidate later file changes or grant publication.
The run's final input verification remains required.

The new actual matched-server chain control obtains CheckDeriveInput's declared
lrr_derive_helper and Empty's captured empty declaration. It checks two native
attribute invocations, unchanged source revision, no derive execution and no seal
or publication. Negative checks reject macro authority denial, a foreign run,
query-provider entry and a different request; metadata replay returns the same
source identity and helper list. The output wire/key/export choices and helper
report are retained. A controlled pure request test checks exact origin/catalog/
occurrence binding. These LRR-specific controls supplement the unchanged licensed
[official rustc cases](../fixtures/upstream/rustc-checkout/README.md); no imported
case is counted as a full native/compiletest/LRR acceptance pass.

The initial missing-API acceptance compile and a Clippy test-lifetime correction
remain evidence. Final counts and audits belong to the checkpoint.

Next work is production discovery/coordinator dispatch and receipt adoption,
helper-scope classification, ordered authorized derive execution/appends and final
positive replay/publication. Original-source and builtin derive payloads, generated
macro-host facets, complete world/edition/Cargo/CLI/service fixtures, M014–M016,
traits/language/JIT/native/security/performance, release and Linux acceptance remain
pending. This component does not complete M013.

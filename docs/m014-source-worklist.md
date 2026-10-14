# M014 captured-source nested declaration work

Task and invariant: the existing source discovery worklist must demand a cold
declaration producer before completing its scope. Ordinary cold operations remain
unprepared; incomplete children and rejected replies cannot become completed
declaration coverage.

The source view scans its already captured parser and cfg input. SourceDiscovery
indexes nested producer ownership and schedules ScanNestedDeclarations jobs in
source order. The existing declaration fragment holds the real discovery ticket
through explicit expansion requests and exact reply admission. Completed output
adds actual impl-header membership and dependencies after preflight, then publishes
its completed-child lexical bindings. Checked accounting includes retained source,
fragment, inventory and publication metadata.

Original item and root annotation producers retain their existing worklist owner.
The declaration index delegates those frontiers; nested lookup still checks the
original owner's lexical readiness. Unknown outer annotations block nested work.
An empty derive can settle without creating a macro record and permit a subsequent
cold expression producer. No annotation is classified by its spelling here.

Non-item grammar records have a separate accessor after source scope completion;
they never pass through item-output admission. The impl inventory accessor returns
owned structural facts and cannot certify complete graph/native-facade coverage.

The isolated draft passes 238 relevant controls across seven binaries, including
49 declaration controls and 14 source-discovery controls. The six new source
controls exercise a cold impl-producing macro alongside ordinary cold type errors,
the unchanged rustc nested-statement fixture, unknown children, outer-annotation
readiness, empty derives and foreign replies. The fixture adaptation uses LRR2024;
full rustc fixture acceptance remains zero.

Authoritative integration validation runs asynchronously. The preceding frozen
milestone has 1733 host/doc passes and 46 upstream component assertions. No new
milestone count is claimed until the integrated run succeeds.

M014 remains in progress. Audited intrinsic argument summaries, procedural and
qualified nested inputs, transformed/dependency source integration, mixed editions,
complete graph/facade impl seals and reached symbolic selection remain pending.
This component does not complete MX029/MX030, guest JIT, release or Linux acceptance.

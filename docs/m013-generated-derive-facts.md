# M013 generated derive discovery facts

Revision 4 Sections 49–51 and Section 57 milestone C; macOS ARM64, Rust 1.99.0.
M013 remains in progress. This component supplies pure discovery facts for the
pending derive/helper worklist; it does not execute or append a derive output.

Invariant: the exact generated output, canonical item, cfg and epoch bind the
compiler-owned derive annotation proof. Paths retain actual parenthesized tokens,
span/context records and editions through the pinned syntax map. Each occurrence
has a distinct stable identity, including repeated paths across annotations.
Unknown replacing attributes, unresolved derive annotations and possible helpers
remain explicit unresolved indexes. No helper declaration is guessed from spelling.
The source resolver must supply exact compiler-owned annotation descriptors;
this data alone cannot certify namespaces, invoke a helper or publish a seal.

The adapter selects original meta arguments by their generated syntax positions,
including effective cfg_attr children. The plan reuses existing pinned-AST ADT
and member cfg facts, so disabled variants do not scan their nested fields and
inner ADTs in constant expressions are not mistaken for the outer ADT's members.
Input sizes, annotation/path counts, retained facts/forests and allocations are
bounded. The derive-path splitter now retains at most 4098 parts before rejecting
an excessive list, preventing an unbounded intermediate split collection.

Language sources now come primarily from the official local rustc checkout,
following the user's instruction. The initial five cases at
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` are copied unchanged into
[the licensed import directory](../fixtures/upstream/rustc-checkout/README.md).
Both licenses, copyright, auxiliary crates, directives and matching snapshots
are retained and verified against Git blob identities and SHA-256 hashes.
The checkout itself is excluded from Git as a local reference; copied fixtures
are tracked project deliverables. `tools/import_rustc_tests.py` reproduces imports
and refuses changed source files or overwriting a different revision.

Four imported cases exercise component assertions through an actual identity
macro expansion over the unchanged copied source tokens. The test envelope has
its own captured source identity and edition 2015; it is not claimed to be the
original upstream source location. The cases cover five ordered derives across
four annotations, disabled fields with a pending helper, cfg_attr helpers on the
item and field, and outer/nested ADT cfg separation. The copied expand-to-derive
case remains unexercised. Six LRR-specific controls cover owned origins/raw and
absolute paths, cfg facts, proof reuse rejection, exact annotation kind and
pending classifications, malformed inputs, and resource boundaries.

An initial test compilation failed on a nonexistent CfgSet constructor; the
actual host API was used without changing cfg semantics. Its failed log is retained.
Full fixture native/compiler modes have not been run for these five imported cases.
No compiletest snapshots, helper execution, runtime behavior or full LRR fixture
acceptance is claimed by the component assertions.

Next required integration: source namespace-selected annotations, exact derive
export/helper declaration selection, materialized ADT/member input, authorized
exact-once invocation, ordered appended output and replacement reanalysis, final
selection replay and complete CLI/service/world/edition fixtures. Existing typed
derive-pending refusal stays required until those demands actually settle.
M014–M016, traits/JIT/native/security/performance, release and Linux acceptance
remain pending. Actual results belong to the checkpoint.

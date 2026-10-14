# M014 recursive adoption integration

This is the next integration plan, not completed acceptance. The existing
`SourceDiscovery` and deterministic `Discovery` worklist remain the coordinator;
the declaration inventory must not mint a standalone impl seal.

The current `ScanNestedDeclarations` branch handles item fragments and staged
modules. An unrecognized nested declaration job stays blocked. The new declaration
scanner and generated macro index do not yet enter that branch. Consequently a
successful nested matcher test does not establish MX029/MX030.

The integration must retain source and generated declaration inventories beside
their existing source providers. Producer jobs suspend with an exact request;
expansion execution remains external. Admission checks the complete key, parent
inventory and epoch, scans the exact output grammar, and enqueues child producers
before completing the parent job. Unknown qualified macros and annotations retain
typed input demands. The existing shared expansion budget must cover these records.

Publication must use the existing textual-scope publication and hygiene facts.
Statement/item output can introduce bindings into its owning scope; definitions
inside a generated block cannot leak outward. Prefix publication must preserve
source ordering and keep unresolved later output blocked. Before any state mutation,
validate child metadata, all registration conflicts and the worklist update.
Wrong or stale admission must retain the original pending request and frontier.

Required acceptance observations:

- The unchanged rustc nested-statement fixture resolves and adopts `grault!` then
  `foo!`, with pending child work observable between them. Original bytes,
  directives and licenses remain unchanged; LRR adaptations remain external.
- A source producer emits a local macro used by a following sibling. Resolving
  that sibling suspends until the producing expansion is adopted. A definition
  inside an expression block remains unavailable to the outside sibling.
- A nested generated macro produces a second nested macro, then an impl header.
  Every level uses actual marked tokens and explicit ancestry; cold unresolved
  ordinary calls remain unprepared.
- Unknown child producers, malformed output, stale replies, budget failures and
  incomplete module/facade coverage prevent completion. Declaration admission
  alone cannot issue an `ImplCatalog` seal.
- Full MX029/MX030 require reached symbolic method selection. Their dependency on
  the subsequent term/goal/dictionary integration stays explicit; an inventory
  assertion cannot stand in for dispatch or guest JIT acceptance.

Mixed-edition nested calls also need a dedicated acceptance case: generated
tokens carry definition and invocation editions separately. The current
`GeneratedDeclarations::admit` check compares the requested call edition with the
parent parse profile. Test actual cross-edition captured tokens before relying on
that check for general generated output. This is an unverified compatibility
question, not a specification contradiction or a language-profile change.

An isolated macOS draft now implements completed-child publication in the existing
source/generated declaration indexes. Its six new controls pass alongside the
prior controls (43 targeted tests total), and strict workspace Clippy passes.
The unchanged rustc nested-statement fixture requires its child producer to finish
before the source frontier can retire. These results are recorded in
[the isolated draft receipt](../artifacts/macos-first/m014-recursive-adoption-draft/validation.json).
The draft is not yet integrated: the preceding milestone still runs against its
original inputs. Worklist integration and full graph/facade seals remain pending.

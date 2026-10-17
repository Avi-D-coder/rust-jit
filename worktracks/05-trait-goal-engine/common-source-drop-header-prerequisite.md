# Source-selected cleanup header prerequisite (source-only)

Bounded item: M019 header/index groundwork for T044/M029, not goal-engine or
Drop acceptance. The unchanged primary witness is
`tests/ui/borrowck/borrowck-assign-to-subfield.rs`: source cdbfba3a, terminal
75e555/1 reached `LRR-E-UNSUPPORTED: cleanup requires its source-selected Drop
witness`. Neither no written Drop impl nor an empty descriptor destructor field
proves NoDrop. Generic/conditional Drop also requires M020/M021/M026 obligations.

The common route is actual captured source/edition/hygiene -> existing pinned
DeclarationScan -> SourceDiscovery -> genuine ScopeDeclarationInventory query ->
new traits/index.rs retained checked locators -> traits/header.rs borrowed exact
source/header requirements. Ordinary/generated scans share this route. No parser,
resolver, runtime operation, assertion handling or source-only exclusion is added.
`Frame::advance_scope_cleanup` remains the continuation owner and its NeedWitness
refusal remains. No alternate executor is replaced by this intermediate transport.

The preparation API is PreparedScopeImplHeaders::prepare(db, &SealKey). Only an
actual Scope certificate is accepted; the provider revalidates source/compiler/
producer/current-epoch dependencies. This is explicitly not an ImplCatalogSeal.
The set retains the actual inner Arc<ImplHeaderIndex>, not a copied index or only
its outer QueryValue wrapper. Every active header is joined independently to one
retained scan header by exact identity, header identity, range, scope, containers,
header tokens and associated header ranges/tokens. Inactive replaced scan entries
cannot be resurrected. Duplicate owners, unmatched rows and checked limits refuse.
The view exposes raw generic/where/associated requirements; names are not converted
into traits/types by spelling. No source-selected Drop trait, candidate applicability,
negative answer, dictionary or executable body is minted here.

Cold cost: checked O(S + N log H + active token bytes) locator construction, where
S retained scans, N scanned headers, H active headers; O(S + H) new fixed-size
metadata. Existing scanner/index roots remain shared. No per-entry/operation source
preparation is installed. Lookup is O(log H); currentness uses the same memo query
and cached outcome fingerprint. New metadata and temporary locators are bounded
before allocation; original index payload remains owned/charged by its existing
query pin. Activation into a new retained query still requires root accounting.
No speed claim or new namespace/world census is made.

Next concrete joins: (1) real configured live/facade unit producer closure must
produce an ImplCatalogSeal, not convert a Scope inventory into completeness;
(2) actual source trait TypeName selection must identify Drop and generic binder/
where requirements through the existing contextual header service, without another
resolver; (3) canonical candidate isolation and required predicate fulfillment must
produce positive/negative/ambiguous witnesses; (4) ordinary cleanup consumes that
witness, runs any selected guest Drop and structural fields exactly once. The A/B
witness remains failing until those joins execute. Explicit shared generic binder
service API must be coordinated before changing frontend ownership.

Evidence required: unchanged A/B runtime, source-selected user Drop, conditional
Drop and generic fields; full catalogue oracle and stale source/epoch/producer
controls; equal handwritten/user/wrapper/standard expansion routes; cleanup,
replacement, panic, suspension and exactly-once effects. No tests or Cargo are run
by this implementation agent. The validator owns execution. Source review and
frozen component fingerprints will be appended after implementation.

## Separate source review, exact component

Reviewed source fingerprint
`651517f74799da67c7f474017cf3e9e7ed29c04a20b1962a29032e4f09aa9e17`:
header.rs `801c0554…`, index.rs `962501df…`. Manifest
`worktracks/evidence/2026-10-16-source-impl-header-transport.json`
SHA `39c019955db9210a3a98484e990ac9048598c0a6ce9bdf585cf458bdf0fcc31d`.
The two-file immutable payload is
`/private/tmp/lrr-source-impl-header-transport-v1/overlay`; no moving frontend or
DB leaf is included. Root must add only public traits header/index module hooks.
Observed checkout HEAD at freeze was 403710ae34d074468ea01df7a337dd16e2e6df62;
component source hashes, rather than that commit, identify the uncommitted payload.

1. Separate assertion/expression evaluator: none added. Header preparation fetches
   ScopeDeclarationInventory through SyntaxProvider and never evaluates Rust.
   Existing unrelated legacy assertion routes remain held; this component removes
   none and does not supply runtime architecture acceptance.
2. Literal/break-only output executor: none. SourceImplHeader borrows actual pinned
   DeclarationScan metadata, including generated origins. No opcode/phase/body
   reconstruction, payload executor or generated-source vocabulary is introduced.
3. Context-specific methods/numeric inference: none. `requirements()` retains raw
   actual impl/associated headers. It does not resolve Drop by spelling, lower
   generic names independently or synthesize a primitive/nominal descriptor.
4. Generated-source exclusion: none. `prepare` admits genuine scope inventory
   independent of source origin; exact retained scan joins preserve owning parse
   profile, edition, cfg, scope/path and generated origin. Executable generated
   Drop/trait contexts are unexecuted, not assumed compatible.
5. Duplicated patterns/eager cold arms: no pattern code changed. Impl/associated
   headers already exclude bodies in the shared scanner; no callee or cold method
   lowering is requested. The untouched Frame cleanup NeedWitness remains real.
6. Repeated preparation: locators are built only in the explicit cold `prepare`
   operation; header selection is binary search and indexed borrow. No installed
   execution calls it. `require_current` fetches the same pure memo query and
   compares its cached outcome fingerprint, without re-encoding a scan. A future
   pure goal consumer must memoize this set rather than invoke prepare per goal or
   destructor; that integration is held, not silently implied by this API.
7. Namespace census/unrelated invalidation: no new namespace lookup/resolver or
   full-world input scan. This scope inventory is an intentionally complete cold
   source contribution, not a per-operation guard or proof of global impl absence.
   Future catalogue/goal query must carry actual relevant enumeration/selection
   dependencies. No precision/performance acceptance has run for it.
8. Immutable copies/node searches: retains the actual inner inventory Arc; scan,
   token and associated buffers are borrowed. New rows contain checked u32 source/
   header ordinals only. Active rows are independently matched once; masked source
   rows are not resurrected. Exclusive allocation and temporary capacity accounting
   are checked, existing QueryPin retains the underlying inventory allocation. No
   parser/body/index cloning, whole-file node search or extra registry is added.

Two external controls are included inline under cfg(test), retaining complete
unchanged originals: four editions of A/B empty scope facts and the four written
user Drop header sources/associated ranges; currentness/epoch rejection and a
caller ImplCatalog key rejection. They are preparation/authority checks, not guest
Drop execution. Whole imported destructor unsafe/global/assert semantics remain
unexecuted. Proposed validator command (not executed here):
`cargo test -p lrr --lib traits::index::tests:: -- --nocapture` on its frozen macOS
source/target with root module hooks. Actual A/B rerun belongs after real Drop
catalogue/name/solver/cleanup joins; no negative refusal is a runtime pass.

Review limits: the new factory is a staged component, not yet an integrated pure
trait-goal query. No M019/T044/M029 completion, full catalogue oracle, user Drop
execution, generic conditional witness, cleanup exactly-once acceptance or eight-
context runtime matrix is claimed. New consumers must retain/account this set's
exclusive rows and its inventory Arc through existing query ownership; no runtime
consumer has been added. Old prototype trait-header adapter remains unregistered
and held, including its original-only/global-world shortcuts; no bytes were copied
from it into production.

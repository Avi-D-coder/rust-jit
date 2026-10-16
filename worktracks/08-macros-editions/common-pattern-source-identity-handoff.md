# Structural pattern attachment and canonical binding prerequisites

LOCAL-024, T024/T050/T051/M014; October15. Source successor to frozen71,
not part of its ongoing validation. The pattern worker owns retained canonical
pattern binding groups and ordinary tuple projections. Root owns normal syntax
attachment, pinned local-name class access and later BodyIndex integration.

Violated invariant: BodyIndex structurally records every actual ast::Pat, but
entered_node_syntax only attaches scalar pattern kinds under a match arm. That
whitelist withholds actual source for tuple, or, at, let/condition patterns even
though attachment itself grants no execution, namespace or binding authority.
The unchanged official or-patterns/search-via-bindings.rs is the immediate
witness; existing original node-lookup bank remains the broader correspondence
oracle. This helper change does not execute or pass the fixture.

Use the same pinned ast::Pat::can_cast classifier for NodeRole::Pattern as
BodyIndex creation uses. Preserve exact body/node/range selection and bounded
Rowan descent; no display parsing, generated-only route or phase bypass. The
existing external oracle compares every indexed source candidate against source
preorder, including the unchanged nested tuple/or/@ original. Runtime recipes
and binding namespace admission remain their existing owners.

Expose ParsedFile::local_name_class over actual identifier ranges using the
already retained pinned HygieneFacts::name_local and existing token-context
attachment. None is only the existing original-source root context, never a
generated-source fallback. same_local_name_context consumes this same helper;
missing/cross-source contexts retain exact refusal. This is a hygienic class
fact for one demanded pattern plan, not definition-site rib adjustment or a
name lookup/type proof. The worker will retain candidate groups once and root
will wire ordinary lexical consumers after its typed API is available.

No fixture-specific semantics, pattern evaluator, type inference, runtime
capability or full static checker is added. Cold result/body syntax remains
unexecuted. No new immutable allocations, global namespace scans or per-phase
plan reconstruction. Existing indexes/Arc facts keep existing accounting.
Review all8 modes against exact successor hashes and validate asynchronously
through the sole validator; the frozen71 source must stay unchanged.


## Root source integration review

BodyIndex retains the worker PatternBindings once after finalized range rows,
charges checked actual storage under its ceiling and explicitly encodes facts
in v5 body structure. Ordinary let and match-arm lexical lookup select a retained
canonical candidate, attach its actual Name and reuse binding_name_matches.
Neither candidate groups nor the lookup helper certify a binding; reached
namespace proof, Pattern transaction and live local cell remain required.
The conditional lookup branch still uses its older condition plan/direct Ident
path and remains a named integration/removal obligation. Definition-site macro
rib traversal through existing LocalScopes also remains incomplete.

Eight-mode review:1–4 use common source identity/hygiene without new evaluator,
macro identity, method/numeric policy or generated-only rule;5 candidates are
shared pattern facts and carry unresolved constants/modes/alternatives;6 one
indexed preparation replaces recursive per-reference pattern discovery, while
existing enclosing statement traversal/condition planning remain held debt;7 no
namespace census or new dependency map;8 actual Arc roots remain shared, ranges
use direct lookup, new fact bytes are checked and stable encoding explicit.

These live root leaves are excluded from V7 validation; Rust controls remain
uncompiled/unexecuted. Exact hashes at this source review:

- `crates/lrr/src/syntax/body_index.rs`: `2dc7cfbd61caa2b7594c4ef3c99a0df15d916c0a44c3deb0d2d6db3aaeebbd4b`
- `crates/lrr/src/syntax/mod.rs`: `7376ae4e5c858fef33f5a2033d52a0fbfc2358c70f2c9debc42f9e533ac40740`
- `crates/lrr/src/syntax/thunks.rs`: `23d5dd48bd8a301b74daedef8a7f401db30d083a53f10b3aa660ec1f37277cbb`
- `crates/lrr/src/syntax/node_lookup.rs`: `544d467f2f003fce9a46e1f3034983e1ba863e19fac7d9a54382899710029ece`
- `crates/lrr/src/syntax/node_lookup_tests.rs`: `76e2ab0f50a2392b526d2f8c34800059cace6f3f32378b14944c564c428a6e1a`
- `crates/lrr/src/syntax/lexical_hygiene.rs`: `4aa41e9f677bbfbdffa0ad6977a6ef946eb699c745c84463f3b182002a4e68f2`

## Conditional candidate join before implementation

Root extends the same retained syntactic candidate consumer to the existing
condition lexical path. Its retained ConditionClause already carries the actual
pattern root NodeId; use it directly instead of scanning condition descendants for
a direct IdentPat. Factor `pattern_binding_group_at` once for let/match/condition.
This removes a structural pattern whitelist only. Existing earlier-clause and
consequent scope checks, actual identifier hygiene and unresolved group refusal
stay; candidate groups never provide Value namespace or live binding authority.
Reached shared pattern admission/transaction must still prove bare identifiers are
bindings, and runtime local reads still require the actual committed live cell.
Bare constant expression lookup and general pattern admission remain held gaps.
The existing condition_plan call still reconstructs a plan during lexical source
lookup; this change does not claim that separate query/index removal obligation.
No new per-lookup plan, parser, namespace scan, runtime opcode or source deep copy
is introduced. Existing unchanged upstream search-via-bindings and scoped
condition/binding controls are the acceptance witnesses, pending combined validation.

The conditional candidate join is now source-only. All three lexical consumers
use `pattern_binding_group_at` and its exact canonical Name attachment/context
comparison. The existing ConditionPlan still determines clause boundaries and
visibility. This removes the condition-only descendant IdentPat selection, not
the separately held per-reference plan construction or Value namespace gate.
Review1–4: no assertion/output evaluator, macro identity, contextual type/method
rule or source-origin restriction introduced. Review5: common retained syntactic
groups, not executable branch preparation or binding authority. Review6: direct
NodeId/group lookup replaces the condition subtree scan; existing plan recreation
remains debt. Review7: no namespace map added; ambiguity/constant classification
remains reached source authority. Review8: same shared immutable index and exact
Name attachment; no new allocation or full-file walk. All MA findings remain open.
The prior thunks hash above is historical; reviewed successor hash: `2f41b09a489fb65f28c3054e8b46d2b665c452fd04fcdd8f15b11183ce8680d6`.
No Rust execution or stock acceptance is established by this source review.

## Ordinary pattern scratch storage before implementation

Product decision lowering needs a distinct ordinary value/place pair for each
actual selected pattern node. Root reserves one pair in the existing
StructuralNode.runtime_temporaries[0], with the following odd slot as its place,
for every NodeRole::Pattern. The same checked slot ceiling and parity schema
apply; this reserves frame metadata without preparing a pattern leaf, source
type, guest cell or cold arm. Runtime expected/test slots stay with the actual
match/condition owner. Pattern worker consumes these ordinary indexed slots;
no macro PC/slot namespace is introduced. Extend the unchanged-original structural
lookup oracle to check unique scratch pairs, parity, bounds and separation from
actual expression slots. Existing metadata fields/explicit encoding carry the
slot IDs; checked extra slot count is subject to normal Frame accounting.
This is a shared prerequisite, not product execution or stock acceptance.

### Ordinary bounded product continuation phases

Root extends `BodyIndex::pc` for actual `NodeRole::Pattern` with ordinary
`Operand(0..=2)` phases. They reserve one charged traversal component, the first
scalar/binding decision and a reached upper-bound decision respectively; they
grant no executable or binding authority. The common product plan and verifier
must still admit the actual operation. This avoids exceeding the eight-op recipe
cap with a depth-dependent tuple projection burst. No macro/raw phase admission
changes. Existing range-marker metadata remains source structural and encoded,
but no longer exclusively grants pattern operand phase zero. Root also adds the
new common `PatternNext` exit to diagnostic tagging and capture classification;
the pattern worker owns its verifier/runtime continuation semantics. Extend the
existing indexed structural controls, including unchanged upstream fixtures, for
the exact ordinary phase boundary. These changes are source-only pending the
sole validator's coherent product snapshot; nominalidentity59 is unchanged.

### Retained semantic structure identity

Root adds `BodyIndex::semantic_identity()`, computed once after final nodes,
source locations and canonical pattern groups are prepared. It is the existing
explicit stable encoding's digest, not an allocation key or a new semantic
encoding. The derived inline Digest is charged under the source-index metadata
cap and deliberately excluded from its own stable encoding. An index is opaque
and immutable after construction. Equivalent independent allocations therefore
compare equal; body/profile equality alone cannot admit changed structural facts.
Product/Frame guards consume this retained digest rather than re-encode the whole
index at construction or each transition. Extend the existing unchanged-original
node lookup oracle to compare retained versus freshly encoded identity and a
second independently built index. This is source-only until product validation;
it does not admit any binding, guest operation or source-origin authority.

### Guard-entry continuation split

Product review found that guarded root success cannot share Arm.Commit with the
post-guard final binding commit/result. Root reserves ordinary MatchArm.Operand(0)
only for actual indexed arms with guard_scope. Guard-entry, final true commit,
false-guard retry and exhausted cleanup remain distinct continuations; no guard
expression or source value is evaluated while reserving metadata. Extend the same
unchanged-original structural oracle for guarded/unguarded admission. Production
switch stays held pending the cached binding/ownership joins and actual validation.

The common pattern transaction needs a distinct bounded cleanup continuation so
retry can retire scratch owners without re-running comparison/binding phases.
The pattern peer owns the explicit IR `Phase::PatternCleanup` encoding at u16::MAX
and schema boundary tests. Root admits it only for actual Pattern/MatchArm nodes
or the actual LetExpr clause owner, preserving normal indexed PCs and rejecting
opaque macro/other expression roles. Operand ordinals now end at MAX-4; reduce the
ordinary operand-count threshold accordingly. Existing macro raw phase-count
limit already yields that ordinal bound and stays unchanged. Source/origin, phase,
transaction and capability checks remain in the ordinary verifier/runtime path.
No new source evaluator, synthetic nodes or macro-only phase is introduced.

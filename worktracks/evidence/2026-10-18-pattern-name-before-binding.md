# Classify a plain pattern name before requiring a binding candidate

Root LOCAL-044, WT04/T033, E-R5-005. V17 actual LRR cold/range/retry control
fails on plain constants LO/HI/COLD in or-pattern alternatives before VM search.
PatternBindings deliberately records BindingOrConstant plus alternative_missing;
PatternTree::Builder::binding nevertheless requires canonical_candidate before
constructing its ordinary Value name slot. That mistakes a syntactic potential
binding for an actual binding and rejects valid constant alternatives. The new
initializer route itself passes unchanged const-meth-pattern and issue-17718.

Retain binding: Option<NodeId> in the same PatternNode::Binding. A plain name
can carry its actual occurrence and namespace slot without claiming a canonical
local. Nonplain explicit bindings retain existing unsupported facts if unresolved.
The one matcher classifies the same actual Value selection first: constants and
constructors use their normal path; only a fresh binding requires Some canonical
candidate before staging. No first-candidate guess, casing/name heuristic, ignored
hygiene conflict or forged canonical identity. The source constraint walker uses
the actual indexed occurrence to ask for constant declaration Type when no binding
candidate exists, and does not publish a local binding for None. Ordinary source
reference lowering keeps its existing canonical/hygiene requirements.

Files: syntax/pattern_tree.rs, vm/pattern_match.rs, narrow pattern walk in
frontend/binding_constraints.rs (worker1/2 own different functions there). Domain
versions advance for tree encoding and for-desugared tree identity. No second
matcher/parser/evaluator, macro rule, namespace census, source planning during
execution or BodyIndex copy. Existing Search/transaction commit/cleanup and
constant initializer demand stay the same. Costs: one Option discrimination at
actual binding stage, no extra query for executed patterns; source-only Type
constraint uses existing EnteredValue occurrence query. No performance claim.

All-eight review: ordinary retained tree/matcher (1-2); namespace identity not
macro/inference context (3-4); only reached actualBinding requires canonical proof,
no changed cold branches/guard retry semantics (5); immutable plan retained (6);
same name/dependency service (7); retained nodes/Arc (8). Required evidence is
unchanged upstream constant pattern and existing false-guard/hygiene controls plus
the actual failed LRR control. Unsupported actual binding conflicts and wider
static-check policy are not solved by this bounded classification repair; no full
pattern/architecture/interpreter/JIT gate closure is claimed.

Source snapshot:
- `crates/lrr/src/syntax/pattern_tree.rs`: `bd4317a7e53d6114f6fc37755bf95b605b8a6e4062a4d99f2559ee2a60239237`
- `crates/lrr/src/vm/pattern_match.rs`: `399264ab472cf4a2e8c6966b48b04b8c99e0e3edc6e22def25e9e35e50b15325`
- `crates/lrr/src/frontend/binding_constraints.rs`: `c63279b1c3b578f14881640e071ad7f175bc7769dd55185cc306a17af4dd92dc`

Only the walk_node PatternNode::Binding arm belongs to this root change in the shared frontend leaf; other changed functions belong to worker1. EnteredValue always queries the actual occurrence, while a proven fresh binding records only its Some canonical node. No expect/unchecked node is introduced.

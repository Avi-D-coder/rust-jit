# M007 expansion-order ambiguity

Expansion-order ancestry differs from syntax-context definition ancestry. The
pinned span source describes both hierarchies independently. A name introduced
by expansion has an expansion origin even when its name token came from a
substituted caller identifier. Inferring that origin from the name context loses
Rust's ambiguity behavior.

`frontend/macro_origins.rs` stores checked explicit expansion parents with a
one-million-row/256-depth ceiling, rejects unknown/conflicting parents before
mutation, walks ancestry iteratively, and fingerprints sorted stable keys.
It selects over a source-ordered list of **already name-compatible** candidates.
A nearer declaration A conflicts with an outer declaration B when neither the
reference origin nor B's origin descends from A's origin. Source is the root;
equality counts as descent. Ambiguity produces a sorted pair and cannot yield a
selected definition through `require_definition`. Unknown/malformed inputs are
errors rather than absence; incomplete producer/name facts must suspend before
entering this stage. Duplicate identities must retain identical metadata.

The native Rust 1.99.0 macOS fixtures establish the behavior:

- Generated after source, distinct generating invocations, substituted name
  and nested block cases reject with E0659.
- Source after generated selects the source definition.
- Two definitions from one output select its last definition.
- A reference within the same output selects that output's definition.

The retained 1.98.1 rustc source's `Decl::may_appear_after` supplied a historical
cross-check of the partial-order formula. It is not a dependency or a claim
that 1.99 compiler sources were installed. Current 1.99 native fixtures are the
actual acceptance evidence. Neither upstream HIR nor rustc resolver code is
linked into LRR. Selected successful rules expand through the actual pinned MBE.
The fixture supplies declaration/order/origin facts explicitly. It now routes
through textual scopes and actual owned pinned name classes before the
expansion-order decision; it is not source-discovery or guest-VM acceptance.

The adapter additionally exposes the pinned name-comparison operation:
normalize for item/local lookup, then call `SyntaxContext::remove_root_edition`
before comparing. This follows the retained `Name`/`HygieneId` comparison rule.
Name equivalence does not erase owner or edition from the original owned records.
Owned comparison-row transport now supplies comparison-only item/local classes.
`frontend/macro_scopes.rs` joins `TextualScopes::candidates` with immutable
original declaration contexts and explicit declaration origins. It filters
by item-name class before the partial-order decision, while validating all
visible metadata. The walk retains every visible same-name binding, nearest
first, and freezes parent visibility at each child opening position. A visible
pending producer in any traversed scope suspends even with a nearer binding;
outer output can introduce ambiguity. Missing/conflicting declaration metadata
is an error, never a fallback to raw context equality or lexical-only selection.
The existing lexical `lookup` remains provisional for component callers.

The seven native cases now exercise this joined lookup and expand only after
it yields a definition. The substituted argument-name case supplies a caller
context and an expansion declaration origin independently. Pure tests additionally
cover incompatible nearer names, captured parent order, origin-aware ambiguity,
outer producer suspension, and rejection without partially overwriting metadata.
Source discovery must publish scope/declaration/name/origin facts together before
serving a complete query; these tests supply those facts explicitly. Macro
path/modular/glob ambiguity, generated exports and full source diagnostics still
need integration before M007/source-discovery acceptance is complete.

E-M007-006 preserves every same-position emitted binding until comparison
filtering. The retained failing witness and regression prevent an incompatible
later spelling from erasing an earlier compatible definition. Producer/name
batches remain bounded by the textual header limit.

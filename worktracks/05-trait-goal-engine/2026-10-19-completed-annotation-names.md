# M019/M020: completed annotation names in the ordinary lexical prefix

Solo /root, LOCAL-049; baseline d6e5ff6804. Unchanged witness:
`tests/ui/coherence/coherence-impl-in-fn.rs`, all four official editions.
The full selected head control currently resolves20/32, not runtime acceptance.

Invariant: completion must reach the same DeclarationNameTables/LocalTypeNames
lookup as handwritten items, including actual output names and unresolved import
or scope effects. Clearing an attribute frontier based on macro spelling, enum
metadata, or a Boolean completion would certify false absence. Metadata already
has CompletedOriginalAnnotation; the cold name publisher currently ignores it.

Extend that receipt with actual completed declaration fragments, authenticated
against the parent's exact adopted occurrence set and SourceDeriveCompletionProof.
Project names through GeneratedHeaderPlan's existing source/cfg/hygiene capture;
retain exact declared-name occurrences and replacement masks. Join projected
names and remaining frontiers into DeclarationNameTables and LocalTypeNames.
Retire original item/attribute frontiers only using those real receipts. No new
name-selection algorithm, AST evaluator, fake scope/root, entered-body grant or
macro-name condition. Preserve unknown/generated imports and unjoined descendant
scope effects as pending, explicitly tracked integration debt. Full generated
Type-occurrence ownership remains a subsequent join, not a completed feature.

Ordinary route: PreparedSourceView -> retained DeclarationScopePlan -> captured
canonical row -> common declaration tables and hygiene -> Type occurrence proof
-> source Type/alias/nominal services. Generated header names join those same
candidate tables and comparison classes; installed VM operations stay untouched.
Preparation remains coordinator-owned and once per immutable source/epoch/seal.
Pure providers only read the published input; no expansion, IO or parse there.
Costs: one cold traversal of completed fragment/output headers, immutable retained
name handles/maps; existing lexical prefix lookup. No runtime source planning,
world namespace census, body copy, resource accounting or new limit.

Evidence: unchanged original PartialEq self-Type head now resolves in all editions,
actual source/body/epoch and opaque publication guards remain, handwritten local
Foo/Drop still works. Keep full32-head assertion enabled and report its actual
remaining generated gaps. Runtime witness remains unchanged Drop main/assertion;
no new distinct stock gain without measured baseline. Review all eight recurring
failure modes separately from MA001-MA008 (particularly MA007/MA008 currentness
and repeated-planning obligations). No task/milestone/interpreter/JIT closure
from a projection helper or successful component alone.

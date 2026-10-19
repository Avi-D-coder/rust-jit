# M019/M020 — completed generated output into ordinary local Type lookup

Solo /root, LOCAL-049; baseline bebda6f57489aa48fc3ffd1caf3632c8472d168b.
Invariant: generated declaration Type occurrences resolve in their actual insertion
scope with the token's pinned item comparison class, using the same ordinary
DeclarationNameIndex and LocalTypeNames as original and entered syntax.

Unchanged witness: tests/ui/coherence/coherence-impl-in-fn.rs, all4editions. Its
original PartialEq head resolves, but actual generated Copy/Clone self heads remain
Pending(SelfType):8missing worlds in the full32-head control. Actual scope/metadata
admission already exists; the cold lexical consumer lacks the completed output join.

Existing route: actual completed fragment -> CompletedDeclarationNameEffects ->
CompletedOriginalAnnotation -> ColdDeclarationNames -> DeclarationNameTables /
DeclarationNameIndex::lookup_prefix -> LocalTypeNames -> ordinary source_types
walk and ImplHeadTypes. GeneratedSyntax/ParseFile and declaration_type_name supply
the exact canonical definition/header/occurrence; GeneratedItems.item_class supplies
the pinned hygiene comparison. No synthesized signature, local scope, original
source range, copied-token structural match or macro/trait-name dispatch is allowed.

Extend completed name effects with exact active record/declaration/scope identities from actual
completed indexes, then bind them to the real original target's lexical ordinal
when preparing ColdDeclarationNames. Pure queries observe the exact derived input,
canonical rows, GeneratedSyntax, ExpansionRecord, seal and epoch. Only a real
completed output bound to this original target can enter its local prefix. Unknown
or other source owners remain pending. Original and generated callers share the
same prefix helper; remove the unconditional original-source-only early return
for these admitted occurrences, retaining unrelated unsupported routes explicitly.

Cold costs: one immutable map entry per active completed output declaration, built once with
the existing name-effect projection. Pure Type preparation performs relevant map
lookup, parser-owned occurrence proof and token-origin comparison, then existing
local lookup. No runtime parsing/planning, new evaluator, world census, BodyIndex
copy, bytes/work accounting, sandbox or static checker. No JIT work.

Evidence required: unchanged full32-head control with actual generated occurrence
and foreign source/header/owner controls; exact output/custody/source/epoch/input
removal controls; prior cold name/shared-plan/whole-runtime regressions and Copy
origin matrix. Source review all8recurring modes, separately MA004/MA007/MA008
(output pipeline/catalog/index audit obligations). No task/solver/runtime gate
closure from header readiness; arbitrary generated attributes/imports, parent
scopes and source-backed upstream solver remain separate requirements.

V308 initial probe builds and passes full32/32heads with16actual generated Type occurrence controls; no guest execution. V309 restricts membership to surviving cfg/replacement headers and adds real input/epoch guards, but fails compilation because SourceRange has no Ord. V310 uses existing scalar range tuples rather than altering the global span type. V308 evidence is not final-source acceptance.

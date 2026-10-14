# M013 generated header registration

The generated syntax bridge exports owned Start/Token/Finish events from the pinned
syntax tree. Runtime reconstruction validates kinds and nesting, replaces the outer
MacroItems wrapper with SourceFile, and uses the existing pinned syntax builder and
outline implementation. Syntax event kinds are internal pinned grammar tags, not
portable IDs, addresses or HIR identities. The original output tokens, origins,
record and exact-key/epoch admission remain intact.

A regression exposed the invalid earlier approach of reparsing the syntax view's
text: token-tree syntax text need not have source whitespace. The corrected path
replays the existing syntax structure. Actual generated function/type registration,
visibility and cfg tests exercise that repair. This is an implementation repair,
not a specification contradiction or frontend change.

GeneratedHeaderPlan validates the emitting owner scope/context and discovery epoch.
Pinned HygieneStore item comparison classes are computed during generated-item
admission; the frontend reads immutable classes to govern ordinary item routes.
 no replacement normalization algorithm is implemented. Existing outline
cfg and visibility rules select active named headers. Their public identities are
GeneratedItemKey fingerprints, with explicit caller-owned unit metadata. Name span
lookup uses a deterministic index. Cold function bodies are not executable-lowered.

Unknown generated imports, macro declarations, nested containers, transforming
attributes or unmatched contextual names retain explicit producer frontiers. They
cannot certify missing names. A temporary integration frontier guards mutation;
it is released only after all planned routes register. Failure leaves a frontier,
so partial facts cannot certify completeness. Frontier count and cumulative
metadata growth are bounded; exact duplicate frontier insertion is idempotent.

Controls use actual pinned hygienic macro output for generated functions, a type
alias, cfg-disabled definitions, private visibility, imports/macros and owner/epoch
refusal. They exercise ordinary namespace registration, not a completed scope seal
or runtime execution of these generated functions. Contextual ordinary lookup,
actual generated import/macro retries, nested mapping, transformed source views,
query seal dependencies and complete MX027/MX028 remain required. M013, full E24,
release and Linux acceptance remain pending.

Fault injection of a mid-registration failure remains pending acceptance; the
current controls exercise successful guard release and retained unresolved frontiers.

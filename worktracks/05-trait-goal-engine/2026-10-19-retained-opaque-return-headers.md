# M019/M020 — retain actual opaque return headers

LOCAL-049, solo owner; baselinee9aef9a7e353eb12fe9f598ba58d247c38234461.
Primary unchanged witness: tests/ui/impl-trait/nesting.rs. V376 executes22VM
dispatches/2source calls, then fails all4editions at an unbound generic U in
the ordinary convert facade. No Into/From-name inference fallback is allowed.

Invariant: opaque return bounds must be prepared from the actual owning Fn
header and retained parser/hygiene/edition source. They are neither a runtime
receiver tag nor a dyn principal with discarded generic arguments. Source
inspection finds that source_types.rs::resolve_actual_type currently groups
ImplTraitType with DynTraitType. This is held migration debt; do not enlarge
it or claim the runtime witness repaired by a new metadata helper.

Extend existing QueryKey::DeclarationGenerics/DeclarationGenerics::prepare
instead of introducing a second parser/query framework or reparsing signature
display tokens. The existing memo already retains exact source/header/context
inputs and the ParseFile Arc. For actual functions, factor the existing common function_header validation
from FunctionSignature; this preserves the namespace-only query contract
without demanding full FunctionSignature/parameter Type preparation. Capture return
ImplTraitType occurrences, parent positions, bound-list locations and actual
impl-token editions by walking only the retained return Type. Skip executable
blocks/closures/declarations; retain unresolved MacroType frontiers explicitly.
An unexpanded type macro is not proof of an empty opaque set.

Expose direct typed AST access using those retained source locations. Retain
the actual generic-declaration query in DefinitionReceipt, so the upstream
fact bridge receives these real headers. Do not create opaque DefinitionIds,
capture lists, substituted bounds, ParamEnv or a ReadyProvider until their
actual semantic/source producers exist. Required explicit/conditional source
facts and four remaining compiler clients still gate root entry.

No runtime/body/source result behavior changes in this bounded step. Existing
FunctionSignature.result_symbolic absence and erased impl/dyn route remain
named until opaque source/hidden/dictionary/member execution replaces them.
This is an intermediate source producer, not language/opaque/M019/M020 closure.

Cost/dependencies: one cold return-header walk in the existing immutable memo;
shared Arc parser/input ownership, direct syntax_at ranges, no body lowering,
new query key/tag, per-operation parsing, world scan or BodyIndex clone.
DeclarationGenerics already binds exact parse/header/context receipt versions;
do not add a new global namespace dependency or claim selective recipe/cache
performance from this component. Only checked representational ordinals;
no byte/node/depth budgets or sandbox/static checks.

Evidence: actual unchanged upstream source in all4editions, original/user-macro/
wrapper contexts using normal admission, real AST bounds and owning editions,
memo reuse, absent/changed source and retained old/new receipt isolation, cold
body queries, nested opaque positions and unresolved type macro frontiers.
Supplemental route controls outside originals; unsupported/unexecuted entries
remain gaps. No stock gain or full architecture gate without actual runtime
source-bound inference/member/solver integration and unchanged main execution.

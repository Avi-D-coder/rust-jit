# T032 accepted source frame definition environments — work in progress

An accepted source callee now receives an owned GenericEnv. It retains the exact
function definition signature, lexical scope, header context, generic parameter
ordinals and kinds, defaults, written bounds and where-predicate recipes. Capturing
these facts does not resolve or prove their bounds. Unused parameters remain
unbound; predicates stay symbolic until a later executed operation requires them.

Symbolic and runtime substitutions are separate maps. A callee T may bind
symbolically to a caller's rigid T while its runtime binding is i64. The callee's
own declared parameter terms remain unchanged. VM generic hints now read the
accepted environment rather than an isolated descriptor map. Caller definition
facts and concrete bindings do not overwrite one another.

Admission checks definition and parameter/result identity, declared type-parameter
ordinals, nonzero descriptors, matching substitution key sets, primitive descriptor
agreement, unresolved inference and structural term limits before making the
owning copy. Ordinal lookup uses a deterministic index. Retained environment sizes
are checked and included in the aggregate call-stack budget before capture. The
environment installs once into its matching fresh frame, before argument delivery.
Existing callable revalidation still guards source revisions at entry and return.

Controls capture missing/unforced bounds and where predicates, lifetime ordinal
positions, caller/callee rigid distinction, immutable ownership of captured maps,
unused parameters, foreign/missing/inconsistent bindings, nested inference and
term depth. Existing interpreted generic hint, local and forwarding controls
exercise the installed environment.

This component covers accepted original-source callees. Full ParamEnv/SourceView/
substitution ID tables, dictionaries and goal fulfillment, entry-root lazy definition
context, impl/Self/member contexts, binders/regions/consts, generated-source and
native/JIT integration remain pending. T032 and release acceptance remain in
progress; Linux acceptance remains pending.

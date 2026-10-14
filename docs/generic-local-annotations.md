# T032 generic local annotations — work in progress

Simple generic local annotations now use the same declaration-parameter hint
identity as generic returns. Annotation lookup prioritizes the owning function's
named type parameters before builtin spellings and preserves lifetime/const
ordinal positions. Hoisted body-local type declarations that shadow an annotation
are explicitly deferred until lexical type namespace integration.

The VM resolves a generic annotation through its accepted runtime substitution
for initializer hints, local declaration, binding and reads. Initialized and
uninitialized mutable locals receive the concrete storage contract; incompatible
initialization or reassignment fails under the existing ownership/type boundary.
A local annotation whose parameter has no prepared binding reports
LRR-E-TYPE-CONTEXT before its initializer instead of inventing a descriptor. An
unbound root generic return remains deferred to the reached-return machine.

Annotation identity admission covers SetHint, DeclareLocal, BindLocal and literal
hint fields. Source recipes retain Parameter(definition, ordinal) and do not
replace the declaration term with the runtime primitive. This does not establish
full slot SourceView IDs or general alias/body-type resolution.

Four controls exercise initialized/deferred/nested generic locals, lifetime
ordinals, a generic parameter named i64 instantiated as bool, initializer and
reassignment rejection, missing local context and local type shadow deferral, and
actual CLI declaration/assignment counts. Existing generic return controls pass.

T032 remains in progress. Forwarding rigid caller views, full nominal/reference/
dyn/native source views, complete lexical alias/projection/const/binder semantics,
member/function-value calls, coercions, dictionaries, context/action/origin IDs,
generated suspension, chunked activation, user destructors and release integration
remain mandatory. Validate macOS first; actual Linux acceptance remains deferred.

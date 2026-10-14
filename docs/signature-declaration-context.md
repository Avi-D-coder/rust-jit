# T032 declaration context and source type names — work in progress

Every original active declaration now publishes its immutable source locator and
exact declaration range/scope. Header/body metadata retains its existing role.
The FunctionSignature query records the context dependency and selects the exact
AST declaration by its range plus header/body identity. Bodyless identical-looking
methods in different traits no longer collide in the declaration search. Reusable
signature semantics retain the scope but exclude snapshot offsets; body-only
changes continue to preserve the signature fingerprint.

SignatureNames pairs the signature with an immutable source view and the existing
NameNamespaces graph. It checks that graph/source publication and definition/
header/scope agree. Definition type parameters resolve to rigid ParamKeys first.
Type-namespace source lookup then precedes builtin fallback. A source `struct i64`
resolves to that source definition; absent unqualified i64 resolves to the
primitive candidate. Aliases/imports keep exact source identity and are not
invented into nominal types or erased into their target yet. Ambiguity, privacy,
missing names and pending producers remain typed lookup outcomes. Owner-local
ribs and Self require their definition context rather than ambient module fallback.

Three registered controls exercise shadowing, primitive fallback, generic
identity, alias imports/qualified paths, ambiguity and unknown macro producers;
bodyless trait declarations have distinct identities/scopes with no body/thunk
queries. A stale namespace/source pairing is rejected. These tests use the actual
prepared original source and pure signature query; no source body is executed or
compiled by lookup.

T032 remains in progress. Alias expansion, owner-local source views, complete
associated/projection/region contexts, concrete signature lowering, RuntimeSubst,
ordered arguments, sealed CallPackets and actual guest-call/return checks remain
mandatory. Macro NeedInput routing and complete generated discovery remain later
integrations; this lookup does not execute a macro or certify discovery. macOS is
primary; no release or Linux support is claimed.

The expanded Self-keyword control initially failed ordinary identifier validation;
the resolver now returns the explicit owner-context demand before that parser.
Both the mechanical test-build repair and the failed Self assertion are retained.
The first gate completed after overlapping that correction, so a separate clean
gate is required for the final source snapshot; it is not used as final evidence.

# Owning local type plan at reached constructors — LOCAL-024

October 16, 2026; M017/M018, T040–T045; E-R5-005 and MA001–MA008.

Unchanged witness: `tests/ui/borrowck/borrowck-assign-to-subfield.rs` fails at
the reached outer `A { a: 1, w: B { a: 1 } }`, before assignments. The existing
ordinary constructor binder selects body-local A but then searches module
outline items for its schema owner. Those module rows cannot prove the local
declaration. It also reconstructs its entered local index at the constructor.

Extend the existing pure `SourceLocalPlan(BodyKey)` query, whose peer-owned
publication retains the genuine body source, sealed macro environment, local
declarations and semantic dependencies. The reached constructor consumes the
actual indexed record Type occurrence through `prepare_body_type`; selected
declaration proof enters the same `SchemaNames` record shape, visibility,
field-order and type binder. The VM construction guard, nested aggregate
storage and ownership transfer remain common. No descriptor, source spelling,
caller NameLookup or synthetic function grants source Type authority.

The declaration-field Type site is captured at its actual parser AST Type in
`syntax/type_schemas.rs`. Its source range is a current-source attachment,
excluded from the nominal schema's stable semantic encoding; current schema
equality, owning body environment and exact declaration source still bind the
attachment. Field lookup must use the same prepared local-prefix mechanism
with the field declaration's own generics. Missing field-site integration stays
an explicit gap, never a module fallback for a local name.

The old public binder remains held for unconverted preparation callers; it is
not a fallback when a published owning environment rejects a lookup. Remove
its original/root reconstruction from production after all root/callee entry
preparation owners publish the common environment. The new engine entry uses
that query before the old outline-owner search, including genuine retained
sources. Never lower unrelated branches or execute a macro in a query.

Preparation and authority: the selected-entry coordinator publishes once;
the pure query memo retains immutable source/index/namespace handles under
the existing accounting and retirement framework. Constructor preparation may
query the selected type/schema at its reached entry, before guard allocation.
Installed operations must not invoke this source preparation. No claimed
speedup; all8 review must inspect actual changed sites and source fingerprint.

Recurring modes 1–5: no macro/expression evaluator, special inference/member
selection or second pattern engine is added. Modes 6–8: consume the retained
plan rather than rebuild the local index; no new runtime world scan or body
copy. Existing broader legacy paths remain held. Closure needs actual whole
upstream main execution, correct nested assignments and genuine cleanup; a
constructor component pass or storage teardown is insufficient. All4 editions,
generated/handwritten/wrapper/standard macro matrix and all8 removal remain
unproved until actual frozen-source evidence establishes them.

Review holds found during implementation: the prepared body Type service still
returns a typed pending result for qualified record paths. Since a published
environment must not fall back to the old route, the candidate can regress a
previously supported ordinary qualified constructor. This must be exercised
and repaired in the common Type prefix service before treating this route as
an integrated replacement. The local field prefix remains a separate pending
peer integration. The source member binder now joins local canonical ownership
through the actual `DeclarationContext.scope.owner`, real `DefinitionBody`,
memoized owning plan and its exact current body-item row, including the actual
declaration token edition. This supplies no new Type name-use authority.

Further source review of `engine/run.rs:2010–2031` (SHA256
`f52adb159be6edf0d7b79414ed51a5c2addb428bd8629105f35b4ac6b7592584`)
found that installed `ConstructStart` execution calls `prepare_constructor`
again at each execution. The frozen seven-leaf candidate still inherits that
recurring-mode-6 violation: a memoized local index alone does not memoize the
complete constructor binding. Integration is held. The next repair must
prepare the source binding through the common pure query/recipe machinery,
retain immutable prepared data with its actual dependencies, and let installed
execution perform runtime guard/state work without re-entering source planning.
A per-frame side cache, source-only identity key or global namespace stamp is
not an acceptable replacement. No architecture-completion claim follows from
a successful single execution of the candidate.

Combined source `c858687b` actually failed compilation with six errors and
executed zero test bodies. Root's existing storage control still used the old
one-argument callback; its correction forwards the exact field occurrence.
The other mechanical failures belong to the owning-query and original-module
components. Original failures remain attached to that frozen source; all
runtime acceptance and the next installed-binding repair remain outstanding.

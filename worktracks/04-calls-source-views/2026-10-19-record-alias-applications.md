# M018/T040 — record alias target applications

Solo root, LOCAL-049 with same-owner LOCAL-037/044/046 interfaces. Baseline
5ac8c991f0162dbe85cf9226b61228a5d9720d5d. Primary unchanged witness:
structs-enums/struct-aliases.rs; static-method-type-alias-11047.rs retains the
separate rooted-call blocker. No extra agent or new claim is authorized.

Invariant: record construction uses the nominal application selected by the
ordinary Type/alias service, including actual alias argument-to-target mapping.
S4<u16>=S3<u16,char> has two target arguments, not the one written alias
argument. Missing/hole/unsolved alias parameters must remain pending until the
common source constraints supply their relationship; never replace an open
term by independent inference holes or forward arguments positionally.

Actual route: ConstructorPlan + SourceLocalPlan generic base prefix -> existing
body_type_declarations AliasContext -> shared prepare_selected_definition and
retained TypeAlias/DeclarationGenerics target AST -> mixed Substitution -> real
nominal schema/SchemaNames record binder -> BoundRecordConstructor -> ordinary
verified construction recipe -> Frame::prepare_installed_nominal_constructor.
Generated source joins at the same admitted owning plan. Frame owns allocation,
operands, exact-once continuation and cleanup. No runtime alias parser/binder.

Extend the shared selected-definition finalizer to accept already selected
source GenericArgs; its existing no-argument caller remains a wrapper. Do the
alias substitution binding before walking the target, so an unsatisfied input
does not prepare the same target twice. AliasContext in the record binder uses
this same finalizer once. Return normalized target argument terms with the
existing binding; the owning prepared record retains their ordinary ExplicitType
materialization forms. Preserve original caller argument proof/query closure.
The VM then sees target arguments with the target schema, never alias slots.
For record constraints, consume that same prepared argument vector instead of
reselecting/positionally forwarding written alias arguments. Other constructor
forms retain their existing source-argument owner.

This stage supplies actually applied alias targets. Inferred/partial aliases,
pattern-name alias selection/application and generic annotations remain named
integration gaps; a closed-target pass cannot close aliases or this fixture.
Do not create a new inference/unifier, expand a macro-specific route, restore
primitive shortcuts or pretend unavailable source/solver facts are ready.

Preparation is cold under the existing BoundRecordConstructor PC/body/source/
epoch dependencies. Target Type traversal/substitution and argument conversion
are linear retained-source work. Immutable BodyIndex/parser handles stay shared;
installed execution reads the retained arguments and runtime descriptors only.
No full-world dispatch scans, source reparse, deep body copy, static checker,
resource cap/accounting or new unsafe. Existing legacy constructor/annotation/
method routes and upstream solver/root integration remain explicit holds.

Evidence: unchanged upstream actual record nodes, exact target/schema identities
and argument order across original/emitted/wrapper forms and all editions;
existing constructor/field/cold and revocation controls; actual main captures
with effects/cleanup/outcome reported distinctly. Inspect source/recipe/VM route,
not only output. Separate eight-mode review at final fingerprint, MA003/MA006
constraints explicitly; no MA001/MA002/MA004/MA005/MA007/MA008 closure claims.
No full task/milestone/interpreter/JIT/performance acceptance from this stage.

V398 preserves one compile error (comparing a fallible FactKind with == instead
of propagating its diagnostic). V399 builds and actual stock struct-aliases
advances from the constructor guard after2VMdispatches to the shared pattern
selector's S2 alias gap after21dispatches, all4editions. It is still a failure.
The source control confirms both selected target applications before failing
an overstrict trace assertion: existing select_path_arguments reads the owning
SourceConstraints syntax metadata by design. V400 forbids BindingConstraints
semantic evaluation and executable/body/callee preparation instead. No witness
is weakened or skipped; source metadata is distinguished from running inference.

V400 passes5/7selected controls. The inherited generic Pair<T> numeric control
finds a real regression: an empty prepared argument list was treated as a new
explicit nominal constraint, relating a dependent inference variable to itself
and making both field literals unconstrained. Preserve the old absent-explicit-
argument behavior when the retained vector is empty. Nonempty normalized alias
arguments still constrain the target. The trace hold also identifies the
existing SourceConstraints provider's memo read of the owning BodyStructure;
that is not preparing a callee or rebuilding a body. V401 allows only a cached
read of that exact owning body, forbids ProviderRun there and other body keys,
and prints offending trace rows on failure. No static checks or holes added.

Next integration priority, from actual runtime evidence: PatternName currently
retains spelling/form/optional lexical NodeId but no complete generic Type
application receipt. pattern_names::pattern_declaration treats TypeAlias as
non-constructor; both select_source_pattern_name and select_pattern_name must
join the same ordinary selected-alias target service before the unchanged
matcher/template binder. For S4::<u8>, retain the actual pattern Type argument
occurrences/owning source/edition and normalize S3<u8,char>; never parse the
PatternName display string or drop fixed/repeated/permuted target arguments.
The source constraint constructor view and runtime pattern selection must share
that application so inferred S4 construction can learn its real target type
from the later pattern. This source identity/application prerequisite comes
before extending pattern matching or generic inference. It is not implemented
or accepted by the present applied-record checkpoint.

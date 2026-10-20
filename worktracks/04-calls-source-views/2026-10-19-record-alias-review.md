# M018/T040 record alias applications — separate source review

Solo root LOCAL-049, parent5ac8c991f0162dbe85cf9226b61228a5d9720d5d.
Reviewed candidate V400 source fingerprint
`737ee3eb071fd3ad9533d9d156dbaa5dc2219f3333051d199e4a45a2ce9b34bb`.
This is an applied-source constructor integration stage; inferred aliases,
pattern selection, rooted calls and complete runtime acceptance remain open.

Actual shared route: SourceLocalPlan::generic_base_body_type selects the real
alias identity through the ordinary body Type prefix. Its AliasContext enters
prepare_selected_definition_with_arguments; the existing no-argument finalizer
now wraps that same function. AliasNormalizer + retain_alias_type retain actual
TypeAlias/DeclarationGenerics/parser/header/scope identity. Required argument
binding precedes target traversal; resolve_actual_type and the existing mixed
Substitution produce the actual nominal target application. No new solver.

constructors::bind_prepared_record_constructor now returns the existing record
binding alongside its normalized target arguments. A private inner binder
updates only that cold local vector after a source alias target is known; it
still uses bind_variant_record_constructor_with_arguments, TypeSchema and
SchemaNames. prepared_constructors::prepare retains the resulting target slots
as existing ExplicitType forms. Original written selections and their query
closure remain predecessors; no caller positions are used as target positions.
The existing VM Frame::prepare_installed_nominal_constructor reads these
retained target arguments and uses its ordinary descriptor/hint/construction
path; the VM is unchanged. BoundRecordConstructor stable value advances to v6.

For record expressions, binding_constraint_nominal::constructed_inward now
consumes that same prepared argument vector. It no longer reselects and
positionally forwards a record alias's written arguments. Other constructor
forms keep the existing indexed source-argument owner. Fact lookup diagnostics
propagate; they are not caught as successful or missing inference.

Audit findings MA003/MA006 constrain context-independent type/member selection.
They are not the recurring mode numbers below, and none of MA001–MA008 is
claimed closed by this stage.

1. No assertion/expression evaluator or compiler is introduced. These are
   declaration Type applications and the existing ordinary record binder.
   There is no assertion identity, synthetic function, fabricated TypeId or
   method spelling in production changes.
2. No literal/break-only macro route. Generated/user/wrapper source enters
   the same source Type prefix, alias finalizer, record schema and normal
   installed constructor. The source matrix is not standard-macro/runtime
   equivalence or full fixture evidence.
3. No macro/context numeric or method selection rule. Alias-to-target argument
   relationships use the existing simultaneous substitution; fixed, reordered
   or repeated source terms are not replaced with inference holes. Record
   constraints use target slots the runtime reads. Existing source inference
   algorithms and incomplete upstream trait/root/member integration are not
   extended or promoted as a completed solver.
4. No generated-source exclusion. Missing alias argument/application facts
   have an explicit AliasApplication demand for every source origin. Direct
   caller parameters retain their actual keys. Compound open application,
   const target metadata and inferred/partial aliases remain named holds;
   unsupported does not mean inapplicable or a passed runtime contract.
5. No pattern duplicate or eager executable preparation. Pattern machinery
   is untouched, and its S2 alias-name refusal is now the next runtime blocker.
   Required alias arguments are bound before walking the target; absent ones
   do not force its Type traversal. No new static borrow/privacy/lint/const
   checker is introduced. SourceConstraints is existing syntax metadata used
   by select_path_arguments, not BindingConstraints inference or guest code.
6. Preparation is cold under existing BoundRecordConstructor and source/epoch/
   environment dependencies. It retains actual selected target Type arguments;
   installed code does not normalize aliases, parse or reselect arguments.
   The no-argument wrapper and supplied-argument client enter the same finalizer
   once. Actual alias target walks outside those memo owners remain broader
   migration debt; no global prepared-once or speed claim is made.
7. No global namespace census in installed dispatch, call entry or guards.
   Alias/nominal lookup uses the existing current source and member services.
   A member index is acquired cold only where an alias target adds Type args
   and the caller had none. Selective alias memo/negative-dependency completion
   and inherited conservative namespace guards remain open. New argument
   vectors are local immutable preparation data, not namespace snapshots.
8. No BodyIndex/parser deep copy or whole-file node search. Retained handles
   remain shared. The wrapper clones only argument terms into the local output
   vector; conversion visits normalized arguments once. The host control walks
   actual body nodes to locate unchanged fixture records; production uses its
   existing indexed constructor Type occurrence and direct retained alias AST.

Cold work is source prefix/declaration/schema lookup, retained target Type
traversal, one substitution and argument conversion, linear in relevant terms
plus existing ordered map lookups. No cap/fuel/time/byte/memory accounting,
additional unsafe, new parser/query framework/backend, compiler/static checker,
IO or macro effects in a query. Existing allocation-overflow checks remain.

V398 preserves a compiler diagnostic for Result<FactKind,Diagnostic> comparison;
V399 propagates the diagnostic and builds. Its source control reaches the two
correct target bindings but fails a mistaken trace expectation banning the
ordinary SourceConstraints syntax metadata. V400 checks BindingConstraints
instead; no executable preparation is admitted by that correction. V399 stock
struct-aliases advances from2VMdispatches to21 and stops at the shared pattern
name S2 alias refusal, all4editions. Static-method alias remains at its original
lexical-callee guard after9dispatches/1source call. All remain runtime failures.

Required final evidence: exact targets/arguments and explicit unresolved alias
hold across origins/editions; existing record installation/cache/revocation,
source numeric/nominal constraints and actual stock-main controls. Cleanup,
exact-once side effects, full patterns, source replacement/suspension, root
solver response, source-selected methods, full milestone and JIT are unproven
unless separately recorded. Source/dispatch counts do not establish a pass.

## V401 source review after regression evidence

V400 builds,5/7controls pass, including actual ordinary record main in4editions.
The new source control's remaining trace assertion also banned the cached
owning BodyStructure memo read made by SourceConstraints. The provider's source
confirms this is metadata reuse, not rebuilding or entering another body.

The failing existing nominal-fields control is a production regression:
Pair<T> without written arguments became unconstrained because an empty vector
was treated as an explicit dependent nominal equation. V401 retains None for
an empty prepared vector, exactly as the previous absent-path-arguments route;
nonempty normalized alias slots continue through the common constraint operation.
This changes no inference algorithm or default policy. The Type/parameter paths
and all eight mode conclusions above remain as reviewed, subject to rerunning
the affected controls. The source control permits only cached owner-body reads,
refuses ProviderRun/other BodyKeys, and prints unexpected trace events; it still
forbids semantic BindingConstraints and executable plans during source binding.

Reviewed candidate fingerprint
`5c42fb6d845a97cdccb668add2765b1c0b3bef4b12a38d2a41bf32fb2bd66d83`,
same parent5ac8c991f0162dbe85cf9226b61228a5d9720d5d. No other source change
is inferred from these host-control corrections. Actual final terminals remain
required. Earlier runtime rows belong to V399, not to this source snapshot.
V400's CLI was byte-identical to V399; its only source difference was the host
trace assertion. No duplicate runtime rerun was used for that test-only change.

Final V401 terminals: library build85692/exit0; CLI build8769/exit0;
seven exact controls12458/exit0; runtime observer68371/exit0 containing8guest
failures. All source/HEAD/test/CLI before/build/after-runtime guards pass at
the reviewed fingerprint. Source application control covers24ordinary bindings
and12explicit inference holds across12edition-origin worlds, with exact target
IDs/argument order and source revocation. Existing record installed sharing,
revocation, numeric constraints, explicit constructor contexts, retained alias
lifetime controls and actual unchanged local-record main in4editions pass.

Both targeted stock alias mains still fail all4editions. Comparing the same
unchanged source hashes/pin/directives against the parent's retained V397 rows,
struct-aliases advances from its constructor refusal after2VMdispatches to
pattern_names' S2 alias classification refusal after21dispatches. Static-method
alias stays at the lexical-call guard after9dispatches/1source call. Failure
captures remain incomplete, with guest streams/cleanup unobserved. No new
stock/runtime/full-fixture pass or refreshed suite total is inferred. Exact
compact receipts and commands are tracked in the V398-V401 evidence record;
raw local logs/builds are disposable. Full task/milestone/solver/interpreter/
JIT/Linux/performance closure remains held.

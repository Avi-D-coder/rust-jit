# M019/M020 source impl clauses — separate source review

Reviewed V384 candidate
`5849c9496e6a932443b037ce98d9a441fe245e2ed1ada856803072af7d75bfb7`,
parent ce42e90240064c3d68717eb86faf37068d80ca74, solo LOCAL-049.
Build/control/runtime terminals are recorded separately. This review establishes
source-path observations, not complete conditional-impl or runtime acceptance.

Audit identifiers are separate from the recurring-mode numbers below. MA003
(assertion-dependent numeric inference) and MA006 (assertion method whitelist)
are relevant constraints on the source-to-solver join: no changed caller reads
macro identity or chooses members/numeric rules. MA001/MA004/MA005/MA007/MA008
execution/index/verifier paths are not changed or claimed closed by this work.
MA002 transactional-pattern repair is not delivered by these Type/Clause facts.
Historical audit status is not evidence that neighboring current paths pass.

Shared route: SyntaxProvider Q71 -> PreparedImplSourcePredicates::prepare ->
existing Q59 PreparedImplPredicates/Q58 head and real SourceBindings closure ->
SourceGenericLayouts -> source_terms::encode_predicate/Encoder -> actual
rustc_type_ir TraitRef::new_from_args and Clause::new. The impl-head route now
uses that same Encoder::trait_arguments; its former local argument-list loop
is removed. Q59 owns actual generic parameter ranges for implicit bounds.

1. **Separate assertion/expression evaluator:** none introduced. Only actual
   source recipes and compiler typed IR change. No VM, assertion operand,
   synthetic signature, macro-name dispatch or expression evaluator changes.
2. **Literal/break-only macro execution:** none. Ordinary/generated source
   joins through existing admitted Q59 declaration inputs, then one Type/Arg/
   Region encoder. Macro expansion, lowering, verifier and VM stay with their
   ordinary owners. Source/IR checks do not claim those full runtime contexts.
3. **Context-specific inference/member selection:** no implementation is picked
   here. Trait identities come from ordinary source name resolution/receipts;
   Self and generic arguments enter the upstream constructor unchanged. Region
   bounds retain direction. Relaxed requirements become no clause. There is
   no MyCopy/Sized-name/Vec/method special case, custom solver or numeric rule.
4. **Generated-source exclusion:** none added. Genuine missing source, generic
   layout and unsupported bounds stay typed pending for every origin. All Q59
   recipes remain retained even when one prevents the typed set from completing.
   No opaque identity, fake candidate, ParamEnv or ReadyProvider is created.
5. **Pattern duplication/eager executable checks:** no pattern code changes.
   Q71 is a separate demand from identity/head queries, so those callers do not
   newly prepare bounds. Region clauses are metadata for upstream type goals;
   no borrow checker or static acceptance/privacy/lint enforcement is added.
   Generic parameter collection visits only the actual impl header list.
6. **Repeated runtime planning:** source recipe/binding preparation is memoized
   under the actual existing impl syntax key/epoch. Retained Q59/Q58/parser and
   generic inputs are shared by Arc. Typed encoding visits retained recipes,
   not source text, and makes no query, IO, macro or guest call. It is currently
   a cold source-fact constructor; integration must retain typed results before
   installed operations. Its two test encodings establish interning/query-free
   behavior, not a hot-path latency measurement.
7. **Full-world hot-path scans/invalidation:** none added to dispatch, installed
   guards or call entry. SourceBindings and existing complete source catalog
   validation remain cold. Context/configuration/world checks mirror the prior
   source-Type producer. Full selective runtime dependency/negative-candidate
   integration remains open; this component does not establish it.
8. **Deep body copies/whole-file searches:** none. Q59 uses retained direct
   syntax_at and shares its immutable source/head. The lightweight Impl AST
   handle is cloned to visit its header list; no BodyIndex/file is deep-copied.
   Implicit Sized wraps a ParamKey/empty-args reference for the common encoder,
   not executable source or a reconstructed plan.

New key/value use explicit tag71; Q59's expanded stable value domain advances
to v2. Source pending reasons are stable encoded through the retained Q59
recipes plus typed SourceTypePending. Existing key/value tags are preserved.
The two exhaustive trace/service classifiers gain only the new metadata case;
they introduce no byte admission, resource cap, sandbox or execution authority.

Cold costs: linear header generic-range indexing, bound/required-declaration
walk, dependency-tracked source queries and typed term encoding/interning.
No production unsafe, alternate parser/query framework/backend, source-name
equality solver or type-tag matching is introduced. No performance claim.

Remaining routes/gates: legacy traits/unify and engine/run_trait_calls selection
are not extended or removed. Actual full source facts, inherited/elaborated
trait predicates, higher-ranked/projection/callable/const/opaque/dyn support,
four compiler clients, solver root/response, source-selected member/dictionary
dispatch and continuation/currentness/cleanup acceptance remain required.
Pending is not an empty constraint environment or NoSolution. The unchanged
runtime conditional fixture must still be measured separately, and a source
clause pass cannot establish that its assertion or main completes.

V381 preserves three compile errors: the private frontend context API and two
exhaustive query classifiers. The fix retains parameter locations at the
existing owning Q59 frontend service rather than broadening private visibility.
V382 builds with no host/guest execution. V383 preserves five host API errors
around actual EarlyBinder::bind(cx,value) and HAS_PARAM flags. V384 corrects
only those host callers and formats the new source module. Final controls and
runtime rows, including any failures, are in the evidence record.

## V385/V386 explicit lifetime-argument prerequisite review

V385 source fingerprint cb184f53d7e210162f558e2d8d8967ade983d7bd26427815ced74c9be1230c90
builds only. V386 candidate fingerprint
e04393f49ecd0b890b4310aa40512d65d098e4ff3ce1d4f35531986165d63ea2
retains the exact same parent commit. The earlier V384 failed control is kept
at its source: the first static-outlives impl was HeadPending because its trait
argument lifetime was discarded/refused; the second had a genuine unsupported
HigherRanked bound. Expected counts and imported source are unchanged.

WrittenTypeDeclaration::region now owns the former source_predicates::region
leaf. The duplicated free function and redundant caller-supplied definition/
generic inputs are removed. Both source bounds and nominal/module/trait generic
arguments consume the same actual declaration lifetime identity. Parameter
rebasing and supplied earlier GenericArg::Region bindings remain visible to
Type defaults; the private default-substitution map now stores typed GenericArg
instead of Type-only values. Supplied complete argument lists keep their early
return after a namespace check; only demanded omitted Type defaults walk AST.

The eight findings remain as reviewed above: this is the existing Type walk,
not a second parser/evaluator; every origin reaches it; there is no trait/method
name matching policy, pattern/static check, hot source planning, global namespace
scan, body copy or whole-file lookup. Lifetime lookup is the existing cold
linear scan of owning declaration parameters; typed substitution uses the
existing map. No runtime lifetime checker or extra resource limit is added.

symbolic_type_pending narrows its region hold only for LifetimeArg nodes that
the shared walk now retains. RefType, other lifetime binder forms, opaque/dyn
identity and qualified function-pointer metadata stay held. Unknown lifetime
names and missing arguments still cannot establish an exact trait reference;
underscore regions retain actual local source positions and remain unsupported
by the exact IR encoder. Const argument Type metadata remains unsupported.
No old erased reference is promoted into an exact proof. Region-outlives IR
remains a solver goal, not a new borrow checker.

Full reference/binder/elision and cross-definition mixed-hygiene/edition cases,
region-dependent omitted Type defaults, completed source invalidation beyond
the exercised epoch guards, suspension, cleanup and actual solver/member/VM
integration are unexecuted here unless separately recorded. In particular,
the changed default-region substitution branch is source-reviewed, not proven
by a neighboring explicit lifetime argument or previous Type-only default.

## V387/V388 retained alias and named-region source review

V387 fingerprint c0de58662b7d1c44e098c6b81e45e4821e39198b5c782e00737ebd40ddfb77d9
builds the retained alias path; no execution is inferred. V388 reviewed source
fingerprint caae970bec2bb5a3e345e80a28a0c353ab96bb96d2c7a80839eb18082e4bd1bc
adds the shared named-region leaf and the unchanged issue-22019 source worlds.
The parent is still ce42e90240064c3d68717eb86faf37068d80ca74.

Affected legacy route removed: deferred_alias_type no longer constructs a
FunctionSignature, reparses displayed target tokens through resolve_written_type,
or filters all non-Type arguments out of its substitution. It joins actual
TypeAlias and DeclarationGenerics source/header/context/generic receipts,
selects the real retained TypeAlias target with syntax_at, uses the common
WrittenTypeDeclaration/resolve_actual_type walk and applies the existing
mixed-namespace Substitution exactly once. This removes the affected alternate
parser route, not all other legacy resolve_written_type consumers.

The common RefType leaf preserves a written lifetime through that same source
region mapper. Elided direct references retain the existing runtime projection;
the exact direct-reference guard remains held. Omitted alias lifetime arguments
get Local metadata at their real declared parameter range, not guessed static
or free early parameters. Encoder::region refuses Local for exact IR. Complete
elision/inference and occurrence-specific lifetime freshness remain open.

SourceGenericLayouts::early_region is shared by generic materialization and
PreparedProvider::source_early_region/Encoder::region. It retains actual source
parameter identity, namespace, index and name from the same immutable query;
there is no second mutable generic store or fabricated name. The source-IR
control now checks actual region arguments against real identity_for_item and
checks aliased reference pointees from the two original GraphWalk impls. These
are compiler representation checks, not trait solving or borrow checking.

All eight recurring modes were rechecked on these changed symbols: no macro/
assertion evaluator, literal-only execution, method/numeric rule or new original-
only exclusion; no pattern/static checking; no parsing in this alias route;
no installed namespace scan/body copy/whole-file AST search. Q58/Q59/Q71 retain
and memoize the changed source-Type demands. Deferred alias target resolution
still visits actual AST when demanded outside those memos by legacy callers;
complete alias-plan retention and removal of remaining token-based consumers
are open migration debt, not a prepared-once performance claim for all callers.
Existing local-alias owner publication holds remain unchanged.

Cold alias work is actual source-query joining, direct target lookup, source
Type/name resolution and one simultaneous structural substitution. Generic
parameter range collection and typed argument matching are linear; declaration
lifetime name lookup retains its prior linear parameter scan. Source map access
uses existing ordered maps. No fuel/bytes/count/time admission, new unsafe,
alternate type solver or speed claim. Full source/epoch replacement, mixed
definition/caller hygiene, standard-macro, suspension/cleanup and actual solver/
member/runtime matrices remain unproven beyond recorded executions.

## Final V388 terminals and acceptance boundary

Both builds and six exact control bodies pass with the same source/HEAD/test/
CLI fingerprints throughout. The clause matrix covers 60 source contexts,
144 own clauses, 12 real higher-ranked holds and 12 relaxations. Retained alias
checks cover 12 contexts; the expanded IR check covers 164 Types/160 TraitRefs
in 60 worlds, including the distinct actual g/h source bindings and identity
argument equality. The existing const/generic and direct-reference/projection
pending controls pass. These counts are source/IR checks, not stock mains.

Thirteen runtime rows were actually observed. generic-fn.rs passes in 2024
with exact bytes and cleanup. Conditional deferred2 fails all four editions
with LRR-E-NAME-AMBIGUOUS (120 VM dispatches/7 source calls). Two extra unchanged
alias programs also fail all four editions: static-method-type-alias-11047.rs
hits the lexical-item callee integration guard (9 dispatches/1 source call),
and struct-aliases.rs hits constructor source/member/type binding (2 dispatches,
no source call). All failed captures are incomplete, with guest streams and
cleanup unobserved; none count as runtime passes.

Inspected failure sites are syntax/thunks.rs's lexical callee lowering guard
and frontend/prepared_constructors.rs::unsupported for incomplete constructor
binding. Those files are unchanged by this candidate. That source observation
is not a measured parent runtime comparison or proof of no regressions. The
retained alias metadata route does not deliver those runtime integrations.
No new distinct stock runtime gain, full fixture pass, full milestone, task,
interpreter or JIT completion is asserted. Source/IR success is useful evidence
for the missing fact bridge; actual upstream root/response and selected member/
dictionary/VM integration remain required before conditional execution works.

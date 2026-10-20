# M018 / M025 — actual frame ownership for written Type templates

Baseline 93fb4b1ff2be52b6942862b564b03a1273f8715a; LOCAL049 with existing
same-root shared interfaces. Unchanged upstream witness:
tests/ui/generics/generic-derived-type.rs, all four editions. Current2024
capture enters f::<isize>, then fails at its Pair<T> annotation after6dispatches,
1source call: annotation Type parameter requires its owning frame hint. The
prior V417 failure remains evidence-bound; do not relabel it as current.

Invariant: source ParamKeys retain their declaration owner, separately from
runtime descriptors and constructor/callee argument ordinals. An annotation or
compound explicit argument must use the existing written-Type materializer,
not a second parser/evaluator/solver or a guessed ordinal binding.

Shared route: admitted BodyIndex and SourceLocalPlan -> Q73 named Type source
application / actual generic nominal schema -> let_annotation_thunk::select ->
retained WrittenType tree -> ordinary HintRecipe::Written and verifier ->
vm::step hint/admission -> current Frame/GenericEnv -> existing WrittenType
find/materialize/bind and PartialHints -> ordinary constructor/call/cleanup.
ExplicitTypeArgument compound syntax uses that same select service, then keeps
its complete symbolic term and actual caller in the existing FrameTemplate.
Original and generated source already join at these shared indexed queries;
keep Q73/schema input guards and exact Type occurrence checks.

Extend the written representation with a root frame-owned template carrying
actual definition and Arc-shared existing tree. Named parameter leaves are
accepted only after checking their actual ParamKey against the owning body;
composite alias FrameTemplates preserve that same owner. Wrap the final tree
once at annotation/cast publication. The source view reconstructs actual
ParamKeys. Runtime validates the current frame/context owner, then delegates to
the existing materializer with that GenericEnv's bindings. Partial/coercion
hints unwrap into the existing fixed-argument context; do not treat them as a
pending nominal construction. Common descriptor finding/binding should accept
callbacks so they do not construct global maps or clone immutable body data.

Remove the unconditional Parameter/FrameTemplate annotation refusal and the
compound explicit argument's closed-only representation. Existing unsealed
components, legacy concrete EnteredType/PreparedTypePlan consumers, complete
solver integration and const/region/provider/cancellation packets remain held;
no other legacy route is promoted. No whole-function/static checks, accounting,
limits, sandbox or native backend changes.

Cold costs: one retained shape/tree and source/schema guard preparation per
actual source query. Runtime visits the retained Type structure to find/intern
physical descriptors and reads sorted GenericEnv bindings; no source parsing,
query fetch, repeated whole-plan construction, namespace census, immutable
BodyIndex copy or unselected executable branch/callee preparation. Fixed partial
arguments are installed once with their hint. No speed claim without measures.

Evidence: asynchronous frozen macOS Rust1.99.0 builds; unchanged original
captures and actual failure progression across all editions, source-owner/
wrong-owner/guard/cold controls, original/emit/wrapper/standard-macro contexts
through ordinary indexed/verifier/VM operations. Any remaining Clone/return/
call failure stays enabled and is not a fixture pass. Review all eight recurring
modes separately from MA002/MA003/MA006 at an exact source fingerprint. Full
M018/M025/card/solver/interpreter/JIT and suite gates remain held.

V426: both builds and5controls pass; unchanged original annotation succeeds in
all4editions and reaches t.clone at12dispatches, then reference caller evidence
reads a raw rigid T even though ordinary ArgumentType already uses Frame::
bound_view. Extend that same existing substitution into ClosedReferenceArgument::
capture before its existing source/value/session/target checks. Preserve the
caller's rigid stored view; only its boundary evidence takes reached bindings.
Both ordinary and operator capture callers share it. Remove the unused public
capture_budgeted wrapper/limit argument rather than retain another admission
entry. No new source-term or inference algorithm. Full original still fails
until real complete evidence exists; V426 remains its own failed receipt.

V427 completes the unchanged original in all4editions with199matched dispatch/
outcomes,5source boundaries,expected10/newline/10/newline output and cleanup;
5controls pass including12actual owner/source/verifier/cold/revocation worlds.
V428 changes only a host runtime-origin control, which fails because its generic
completed helper omits standard facade sources (CLI acceptance explicitly uses
LRR_EMBEDDED_FACADE=1). Use existing format_trait_facade_tests::run's explicit
capture_with/standard_facade_crates/standalone_captured world for this test;
never mutate process env or special-case Clone. Preserve the failed receipt.

Source review also finds remaining common WrittenType VM consumers: pointer
casts and nominal stated constructor hints. They must use Frame's same checked
materialization join for frame templates; leaving a closed-only bypass would
narrow the new representation. Keep the pointer target's actual frame owner in
ordinary IR verification as for annotation hints. This is the same mechanism,
not another pointer or constructor evaluator. Recheck retained pointer controls
when changing those call sites. Remaining inference-hole/const/region/runtime
owner-negative matrix entries stay explicit holds until exercised.

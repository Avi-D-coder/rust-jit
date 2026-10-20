# M018/M025 frame-owned annotations — separate source review

Baseline93fb4b1ff2be52b6942862b564b03a1273f8715a; V426 exact source fingerprint
3951a73a799d07c26895471cb0b8850125c35b431aa41964ee820c4878ec69fb.
Initial V425 compilation fails: the newly recursive closure in source_with
caused unbounded compiler monomorphization. V426 uses the same dyn callback
contract already used by the materializer; no recursion limit was raised.
Runtime acceptance pending. MA002/MA003/MA006 are distinct audit obligations.

Inspected let_annotation_thunk::{thunk,select}, entered_type_thunk::coercion_cast,
explicit_type_arguments::{select_argument,ExplicitType::descriptor}, Q73 source
application/site checks, WrittenType::{in_frame,source_with,find_with,find_in,
materialize_with,admits_with,bind_with,exact,coercion_free}, frame_written_types
helpers, step::{frame_hint,materialize_hint,written_binding_admission},
frame_slices::written_unsizing, frame_partial_hints::{set_partial_hint_with,
definition_result_partial_hint,partial_exact}, IR verifier annotation checks.

Named ParamKeys are reduced to an ordinal only after their owner matches the
actual indexed source body. Alias FrameTemplates use the same owner check;
their existing tree is reused. The final annotation/cast wraps that whole tree
once in WrittenType::Frame(actual definition, Arc template). Source reconstruction
produces actual owner ParamKeys; runtime descriptor preparation validates both
Frame body and current GenericEnv definition before lookup. The wrapper is
representation metadata; its inner tree still uses the one existing materializer
and shape/binding engine. It cannot materialize or bind on a constructor's args
without the frame join. Compound explicit arguments keep the same complete source
term and existing FrameTemplate representation. No independent inference or
trait-solving algorithm, candidate selection, canonicalizer or cycle engine.

1/2. No assertion evaluator, generated scalar/break route or macro-only opcode.
Changed annotations and explicit arguments share the ordinary admitted Type/
shape selection and HintRecipe::Written/VM route. Origin runtime matrix pending.
3. No context-specific method selection or numeric inference. Type parameter
identity selects a current binding, never a name/macro/fixture whitelist. Existing
source/currentness checks are not replaced with runtime spelling lookup.
4. No generated-source exclusion or source-fact fabrication. The same owner
proof applies to all origins. Exact Q73 Type sites/guards remain required.
5. No new patterns or static/borrow/privacy/const checks. Reached hints do not
prepare unselected branches, other arms or unrelated callee bodies.
6. No source planning/parsing/query fetch in new frame helpers. They visit the
retained written tree using existing descriptor operations. find_with avoids a
per-use argument-map reconstruction; source_with's dyn callback prevents compile
time monomorphization growth. Runtime GenericEnv lookup is existing sorted lookup.
7. No full-world namespace census or broad new invalidation. Q73/schema/caller
source guards remain the relevant input closure. Root templates retain actual
owner identity and stable nested-tree encoding (WrittenType domainv2, fresh13).
8. No BodyIndex/parser deep clone or whole-file syntax search. Templates are
Arc-shared across frames; closed annotation trees keep the old representation.
Existing selected-leaf/alias tree cloning remains cold. Partial hints capture
fixed runtime arguments once at installation using actual declaration ordinals,
separately from construction-guard arguments. The old result-hint fixed argument
loop now shares written_frame_arguments rather than adding another builder.

The verifier rejects a root frame template with another body's owner. Runtime
checks owner again independently. Wrapped trees are emitted only at their root;
foreign/unavailable source applications stay explicit errors. No resource
accounting/budgets/caps, sandbox, unsafe or backend choices added. New shared
wrapper accounting returns0 under E006. Existing inherited accounting/legacy
source routes and broader solver/currentness/cancellation debt remain named holds.

Acceptance requires terminal builds, original capture/failure progression and
focused controls. A source component cannot close M018/M025, runtime/JIT or
full-fixture/diagnostic/tooling compatibility. Full original failure stays a
failure even if a reached annotation now succeeds. No speed/scaling claim.

V427 source review extension at59e46c24cfdab254340d9d7f5d8a56cc5a685f219d5a1c1a3b6e30ed1ed994cf:
ClosedReferenceArgument::capture now shares Frame::bound_view with ordinary
ArgumentType and operator argument preparation. It retains the actual caller
region and owning-frame source, then performs the same source/value/session/
target checks as before. The stored caller view stays rigid. Capture's proof
source and the ordinary argument source now agree on reached substitution.
This adds a Type-term walk/clone per captured reference, not source parsing or
an inference algorithm. No constant-time claim. The unused public budgeted
wrapper is removed; ordinary/operator consumers call the same capture entry.
V427 source control passes12worlds: actual Pair<T> source owner, ordinary
SetHint/verifier, foreign owner refusal, guard withdrawal/cold, and refusal to
instantiate the root frame wrapper from a constructor argument list. Both
builds/5controls and all4unchanged original runtime captures pass. The old
FrameTemplate descriptor branch is now reached by g::<Pair<T>> in that original.

V428 changes only host origin regression code. Without explicit standard facade
sources its helper fails at Clone, so it does not establish an interpreter
regression or a runtime origin pass. The new test must use the already existing
explicit standard facade capture helper, matching the CLI acceptance world.
V429 fails compiling the first qualified helper module path (it lives under
cli::crate_graph). No guest/control pass is claimed for either failed attempt.
V430 source fingerprintb13fb9d2aeb3c679c2d665f2fced34794837323c6f923d04b1a95b3f192c1011.

V430 review additionally inspects Frame::pointer_cast and nominal_args fallback:
these remaining generic WrittenType materialization consumers now use the same
Frame::materialize_written_hint, rather than rejecting its frame-owned wrapper
through the old closed-only operation. PointerCast verifies its actual template
owner as well as pointer shape/regions. Its inferred-pointer fallback remains
unchanged for ordinary *const _; unsupported compound inference holes are held.
No separate pointer/nominal evaluator, name lookup or source planning is added.
The original eight-mode conclusions still apply to these two changed consumers.
Full generic pointer-template runtime coverage beyond retained pointer controls
is pending; no universal annotation/coercion compatibility claim is made.

Final V430 terminals: both builds0;3/3controls0. All16runtime origin/edition
worlds execute ordinary prepare/seal/parameter opcodes:5/5/3 for original,
emit and wrapper,8/8/6 for the additional standard-assert probe. Expected guest
output is10/newline/10/newline and no stderr. Actual source/HEAD/binary closing
guards match. V430 maintained owning2015 and supplemental2018/2021/2024
original captures complete at199dispatches/outcomes,5source calls and cleanup.
The additional align-struct original remains a failure at7578dispatches/22calls;
no complete-case or baseline regression claim is made for it. V427 source-owner
controls remain attached to their own fingerprint; the final extra pointer
consumer/owner checks were separately reviewed and pointer controls rerun. Full
M018/M025, solver, interpreter/JIT and full-fixture/suite gates stay held.

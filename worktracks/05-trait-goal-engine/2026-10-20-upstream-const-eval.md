# M020 bounded pinned constant-normalization client

Baseline `a3e36397f8dd26c0bc5797d7e42908bce0726a26`, owner
`/root/wf_value_helpers` delegated by implementing lead `/root`; macOS1.99,
all owning source editions. Scope: new `infer/const_eval.rs`, new
`tools/upstream_const_eval.py`, this record and necessary exact pinned compiler
originals. Existing delegate/generator/model/prepared APIs and Cargo/test/commit
ownership remain with lead/validation worker. Existing parent claim and
well-formed-goal reservation govern; this adds no external issue/PR identity.

Invariant: demanded `SolverDelegate::evaluate_const` must execute pinned
`rustc_trait_selection::solve::delegate::SolverDelegate::evaluate_const` and
`traits::try_evaluate_const`, not a function-pointer substitute or a reduced
constant-shape policy. Pin is `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
The parent upstream repr128 runtime witness stays unchanged; component client
completion cannot establish a runtime gain or finish M020.

Common route: existing guarded source/header/generic/ParamEnv and actual typed
constant facts -> shared TypeIR/inference -> next-solver delegate -> pinned
try_evaluate_const and placeholder folder -> immutable terminal compiler-query
result -> shared Const/goal handling. Original and generated syntax join before
TypeIR. This client parses no source, enters no ordinary function body and
executes no guest initializer. E-R5-007 keeps reached const/static initializers
with the ordinary VM; the pure preparation contract is unchanged. E-R5-006
allows no new work/time/memory accounting or count ceiling.

The existing `solver_clients.evaluate_const` function pointer is the alternate
route to remove in lead integration. Port all Value/Error/Param/Infer/Bound/
Placeholder/Expr/Alias branches, GCE abstract expansion and unused-generic
handling, repeat-count identity args/owner environment, MCG and other alias
postponement, region erasure, normalization callback and error mapping. Existing
compiler-derived abstract expansion, region erasure, identity arguments,
deep resolution, error propagation and alias Type lookup remain shared.

Transitive client helpers include the exact compiler placeholder folder,
InferenceContext::typing_env and TypingEnv construction/post-analysis selection.
Pinned next-solver ParamEnv::with_normalized is identity; no legacy eager
normalization algorithm is introduced. `param_env_normalized_for_post_analysis`
reads the actual owner's ParamEnv and that same identity rule. Compiler
instance resolution/CTFE query results are an explicit typed immutable boundary
because their full MIR/compiler infrastructure is not this adapter. No callback
invokes rustc/cargo/check, performs IO/macros, executes guest code, returns a
placeholder None or fabricates query results. Terminal missing query data and
owner ParamEnv remain typed preparation dependencies, separate from real
HasGenericsOrInfers/TooGeneric/normalization/evaluation failures. Admission and
the actual pure source-result producer remain lead-owned integration holds.

Preparation dependencies bind actual definition, generic args, environment,
source/target/epoch and query provenance before context entry. Hot client work
is retained-IR traversal/folding and exact keyed lookup; no namespace census,
parser, BodyIndex copy or complete-plan reconstruction. Normalization callback
is FnOnce, used only after successful terminal value evaluation as upstream.
No performance claim follows without measurements.

Evidence required: exact licensed pinned originals/hashes and generator audit;
separate source-aware eight-mode review at source fingerprint; lead-owned frozen
macOS builds and source/inference controls including generic postponement,
actual error/invalid-type/normalization outcomes and GCE/repeat owner-env
branches. Actual complete source/terminal-result/root/member/VM joins and
upstream runtime/origin/currentness/cleanup matrix remain held. No Cargo/source
or runtime test is authorized to this worker.

## Frozen source-only handoff and review

Final generated source SHA256
`c8089b74c25758cdae9a8f93c9287f97d38d0cb901751115554fab3056999647`;
generator SHA256
`9ba729ee9cdcf1ae1926931e5634e88bda5635d391866e371bceaa701a032e01`.
`python3 tools/upstream_const_eval.py` terminated exit0 with matching registered
transformations. Separate provenance check used `git -C rustc/rust show
<exact-pin>:<path>` and compared SHA256 and retained vendor bytes for all six
input files; exit0. These are source/provenance checks, not Rust execution or
source-aware acceptance tests. No Cargo command was run by this worker.

Originals selected by the generator:

- `compiler/rustc_trait_selection/src/solve/delegate.rs`:
  `ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095`.
- `compiler/rustc_trait_selection/src/traits/mod.rs` (45728bytes, newly retained):
  `49a807c38f798fd55fbe908399114f6707ff480d66bfacc1416ddecc313d5cc8`.
- `compiler/rustc_infer/src/infer/mod.rs`:
  `0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327`.
- `compiler/rustc_middle/src/ty/mod.rs`:
  `bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7`.
- `compiler/rustc_ty_utils/src/ty.rs` (15698bytes, newly retained):
  `0d1edae3ada2b3186448a44c151a6d07916461b781f1803edb9f1e5f6bd5afee`.
- `compiler/rustc_middle/src/mir/interpret/queries.rs` (12150bytes, newly retained):
  `a56f205b0cdef592e53936b1d912af058231c04f81f4f89761247b39167c98f4`.

The source-only review inspected `evaluate_const`, `try_evaluate_const`,
`replace_param_and_infer_args_with_placeholder`,
`InferenceContext::typing_env`, `TypingEnv::{new,fully_monomorphized,
post_analysis,with_post_analysis_normalized}`,
`param_env_normalized_for_post_analysis`,
`DbInterner::const_eval_resolve_for_typeck` and immutable query accessors.
All eight recurring failure modes were considered separately:

1. No expression parser/evaluator or assertion compiler. The compiler's client
   algorithm reads shared IR and immutable query outcomes; terminal CTFE is
   not emulated by a new evaluator or callback pointer.
2. Every compiler ConstKind/anon-const category is retained. No literal-only
   generated executor or scalar-shape workaround is added.
3. Generic postponement, GCE abstract-constant handling, repeat-owner env and
   FnOnce Type normalization follow source branches. Macro identity and member
   names do not occur in selection or constraints.
4. No original/generated exclusion; the client accepts the same TypeIR family
   after the existing shared source-identity join. Full origin matrix remains
   unexecuted here.
5. No pattern machinery or traversal/preparation of unselected executable arms.
   Expansion folds only the demanded abstract constant as upstream.
6. No source parse/plan reconstruction. Deep resolution/region erasure/folding
   reuse existing retained IR clients. Their cold traversal costs remain
   proportional to visited demanded IR; map access uses exact hash keys.
7. No namespace scans/publication. Owner ParamEnv/query leaves are immutable
   indexed inputs; missing records return typed dependencies, not a semantic
   negative answer. Actual source-epoch query closure remains unimplemented.
8. No BodyIndex, source-tree clone or whole-file search in execution. Small
   TypeIR handles and query records are copied; retained values remain shared.

The terminal compiler method's actual no-non-region-infer and no-InherentSelf
preconditions are copied before the data boundary. Instance resolution, actual
pure evaluation/valtree generation and their complete transitive dependencies
are not implemented by this component. The immutable record represents the
complete terminal `const_eval_resolve_for_typeck` outcome, preserving its exact
input TypingEnv, alias, resolved owner definition and definition span. Actual
query producers must not synthesize successful leaves or use TooGeneric to
represent unavailable data. The compiler's post-success static lint inspection
is outside this language runtime client under E-R5-006; no lint enforcement,
node accounting or CTFE resource cap is added. Real reported guarantees/spans
and TooGeneric outcomes are distinct terminal records. No diagnostics or
compiler-tooling compatibility is claimed.

Integration API:

- Lead registers `infer::const_eval` and removes the former
  `SolverClients.evaluate_const` callback route in its generator/delegate.
- `PreparedProvider::const_eval_inputs(self)` returns
  `&'db PreparedConstEvalInputs<'db>`, stored as a required prepared-facts
  reference; no default/fallback provider.
- `PreparedConstEvalInputs::new` takes actual maps of owner DefinitionId to
  ParamEnv and `ConstEvalQuery` to `ConstEvalResult`. Construction itself grants
  no readiness or source validation. The real producer must observe exact
  source/world/target/dependency and terminal query provenance before entry.
- `ConstEvalQuery` fields are `typing_env`, `definition`, `constant`, `span`.
  `TypingEnv` reuses shared `TypingModeEqWrapper` Eq/Hash semantics. Its
  `fully_monomorphized(cx)` adds only the context argument needed to intern
  the same empty ParamEnv; it preserves compiler Codegen mode.
- `ConstEvalResult` is `Result<Result<ValTree,Ty>,ConstEvalError>`, retaining
  valid value, non-valtree Type, Reported(ErrorToken,Span) and TooGeneric(Span).
- `evaluate_const(infcx,param_env,alias,normalize_ty)` returns
  `Result<Result<Option<Const>,E>,ConstEvalDependency>`; outer Err means only
  `PostAnalysisEnv(id)` or `Evaluation(query)` is missing. Inner outcomes map
  exactly as upstream: value/error Const, postponed None, or normalization E.
  Direct callers may report the typed missing dependency. The upstream trait
  signature cannot expose external source preparation, so an unadmitted
  delegate call is an explicit invariant failure; root admission/suspension
  integration remains held, never reduced to None/NoSolution/fabricated E.

Runtime/stock fixtures and source-aware branch controls remain unexecuted by
this worker; the validation worker owns builds/tests after integration/freeze.
The exact helper bodies and registered boundary do not establish an actual
terminal producer, complete root invocation, runtime behavior or gate closure.

## Supplemental actual-value control scope

Lead authorized a test-only helper after the production source handoff. It
accepts an already-encoded actual-source `ConstKind::Value`, invokes pinned
`try_evaluate_const` directly and requires the identical returned Const with no
normalization callback. Parent owns selecting/extracting the array length from
unchanged upstream fixtures and creating the actual source context. The helper
supplies no fake alias, provider, guest expected value or source encoding.
A separate raw-data-boundary helper can accept an actual `ConstEvalQuery` and
assert that empty immutable maps return exactly `Evaluation(query)`; it does
not call an evaluator/delegate or grant complete-provider readiness. Missing
owner-env checks likewise accept a supplied real definition identity. A direct
Alias client branch stays unexecuted when actual alias metadata is unavailable.
These checks are supplemental API/assertions, not a new miniature language suite.

Lead refined the supplemental control to actual symbolic array lengths as well
as values. Final APIs are `check_actual_non_alias(infcx,env,value)` and
`check_missing_owner_env(actual_definition)`, both cfg(test). The former
observes the compiler's deep-resolved non-alias/non-error input and checks Value
returns the same resolved Const, while Param/remaining Infer/Bound/Placeholder/
Expr yields HasGenericsOrInfers; neither path may call normalization. Parent
owns invoking this on unchanged actual-source arrays, including parameter N.
The latter uses empty raw data maps solely to assert the exact missing actual
owner dependency. No terminal-query/Alias helper was retained and no synthetic
alias/query/provider is created. Actual Alias/terminal/GCE branches remain held.

Supplemental final source SHA256
`509bbf12a0069f7be0232b79a06d0fdc08457a3d35460e53648836f96a635ed5`;
generator SHA256
`b17e7664754fdd15cf1fb6f84c99cc7eb333386aaa7094a92e72a5960d0a52a5`.
Generator audit exit0. A Python comparison removed the registered CONTROLS
suffix from generator output and compared the rustfmt-normalized production
prefix, confirming this addition changes only cfg(test) helper source; exit0.
Eight-mode production review remains attached to its earlier production
fingerprint; these helpers introduce no production semantics or parser/source
values. No Cargo or source/fixture execution was run by this worker; parent
and validation worker own invocation and terminal results. Worker freezes here.

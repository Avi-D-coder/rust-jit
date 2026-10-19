# M020 — actual complete InferCtxtLike client and generalization

Solo root/LOCAL-049/WT05-02; baselinef38017f45c48e9fe05f397e6ba25d5bc22cd84cc.
E-R5-014/spec53.0 requires the real upstream inference client before delegate/root
execution. Previous actual storage/regions/binders/opaque/outlives/status pieces
do not implement InferCtxtLike. Do not fill its missing methods with no-op region,
opaque, unification or empty predicate callbacks. Runtime witnesses remain
unchanged methods/method-two-trait-defer-resolution-2.rs and traits/multidispatch1.rs
source worlds; component controls do not execute their original mains.

Reuse complete pinned infer/context.rs trait implementation and lower-universe
folder plus relate/generalize.rs, selected missing readonly table accessor and
opaque subtype lookup owner. One existing InferenceContext/inner/combined undo
owner, actual rustc_type_ir TypeRelation/PredicateEmittingRelation and folders.
No independent generalizer, canonicalizer, relation or solver. Source-ready guard
remains; a compiled InferCtxtLike does not grant full facts or SolverDelegate.

Transport actual standalone no-Z profile flags through existing source settings:
compiler InferCtxtBuilder default overflow FCW true (infer/mod.rs:611), and default
disable_fast_paths false (rustc_session/options.rs:2473). Root currently admits no
-Z flags; Cargo/override flag profiles remain a gap, never guessed. New solver
selection uses the shared InferCtxtLike documented default true for noncompiler
clients; no legacy solver. TypingMode remains caller supplied. Span data adapter
retains actual incoming spans and preserves explicitly upstream DUMMY_SP/FIXME
where the original API has no span; full diagnostics/origin fidelity stays held.

Representation joins: actual Ty/Const/Args/Cx/DefId families, concrete source Span,
compatible collector lifetime order/current interfaces, optional known-region
span data, source generic/alias constructor APIs. Keep original branches, occurs
checks, alias/variance/universe behavior, taint, retries and failures. Register all
mechanical projections with exact original hashes/licenses. No local unsafe,
resource budget, sandbox or full static/body/borrow/privacy/lint checking.

Ordinary/emit/wrapper/source editions share Q69/Q70 -> Encoder -> actual Cx ->
common inference operations. No parser/query/IO/macro execution/global namespace
scan/BodyIndex copy or VM code changes inside callbacks. Generalization/folding
costs remain actual cold upstream work; no speed or scaling claim. Real source
metadata still must be prepared before callbacks demand it; no fake providers,
opaque IDs or full-fact success tokens to get a convenient first goal.

Required evidence: original/source projection checks and macOS1.99.0 async frozen
builds; actual source matrices using the real InferCtxtLike API (fresh variables,
raw instantiation/universe lowering, region spans/constraints/assumptions, snapshots
and existing operations), actual generalization occurs/universe/variance behavior
where demanded facts are ready. Use upstream operations, not a synthetic relation
that emits fake success. Nonempty source opaque/source function binders, nominal
variance/alias/full facts and actual delegate/root/VM joins remain explicit gaps.
Review all8 recurring modes and separate MA findings. No full M020/interpreter/
runtime/JIT/Linux/diagnostic/performance acceptance merely from interface compile.

## Mechanical joins discovered during build (V353/V354)

V353 failed with38 compiler errors; V354 failed with3. Keep both terminal/source
manifests. Identifier-level bug! conversion is required: substring replacement
also corrupts debug!, as the first failed candidate demonstrated. Concrete
shared types expose VarianceDiagInfo through relate, Term::expect_ty through the
inherent trait, TyVid through ty, and Ty::kind by value; imports/paths/pattern
spelling adapt these wrappers without altering a branch. Known-vs-Unknown const
assertion uses the compatible enum's known().is_none() predicate (same cases).

The actual compatible storage is reached through Deref. The compiler's
self.probe/self.commit_if_ok/self.universe/self.create_next_universe forwarding
must qualify InferenceStorage, because the new trait method otherwise shadows
that owner. No new snapshot implementation. Region/type/const creation takes
(universe, span) on this client instead of compiler (origin, universe).

Type-variable diagnostic origins use the compatible table's var_span, which
returns the actual retained span of the subtype-unified group. Compiler
var_origin indexes original per-variable values that this compatible table does
not retain. This is an explicit diagnostic-origin fidelity gap, not a change to
unification, universe, occurs or generalization logic; exact diagnostic parity
is unclaimed. Const origins retain their existing actual span representation.
The compiler's own no-origin DUMMY_SP/FIXME is retained; no fake source owner is
registered. A future exact diagnostic implementation must preserve original
per-variable origin records, rather than relabel these as original origins.

Shared SolverRelating/RelateExt (vendored type_ir/relate/solver_relating.rs) is the
supplemental control relation. No fake PredicateEmittingRelation or SourceFacts
provider. Empty caller bounds explicitly scope structural relation controls;
they do not claim construction of the fixtures' trait-root ParamEnv. Real source
self types/spans come from the existing unchanged upstream Q69/Q70 matrix.

## Actual unchanged runtime witness (V356, not a pass)

`target/debug/lrr --edition 2015 run fixtures/upstream/rustc-checkout/tests/ui/traits/multidispatch1.rs --runtime-capture`
exits2 with LRR-E-CATALOG-INCOMPLETE before observation: std::fmt::Debug is absent
from the default world. source/embedded_crates.rs deliberately stages facade
capture behind LRR_EMBEDDED_FACADE=1; no source import is removed or fixture edited.

With that existing environment flag, the exact same command exits2 with
LRR-E-NAME-AMBIGUOUS: "several impls provide `get` for the receiver and its argument
types". Runtime capture reports31VM dispatches/outcomes and1source-call boundary,
then fault; capture incomplete, guest streams unobserved. This is an executed
runtime failure, not a runtime-contract/full-fixture pass. No zero-output success
is inferred. The ordinary source-bound M:MyTrait<T> plus reached T=usize/u8 must
reach upstream selection through the actual ParamEnv/trait-arguments/response/
member join. Do not choose an impl by name/order/expected output or add an
assertion-only result constraint. Genuine SolverDelegate/canonical instantiation,
complete relevant source facts and common call integration remain required.
Default facade admission and configured runtime acceptance are separate gaps.

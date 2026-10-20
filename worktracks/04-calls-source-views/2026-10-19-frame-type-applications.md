# M018/M025 — shared frame-bound explicit Type applications

Solo LOCAL-049, baselineab1000a9ec. Primary unchanged witnesses remain
consts/issue-58435-ice-with-assoc-const.rs (Self=S<T> in id), self/self-impl-2.rs,
and generics/generic-derived-type.rs (Pair<T> annotation and explicit g argument).
The latest source matrix retains the actual S<T> term but Q51 currently calls
closed_written_type, so it discards that open application. ExplicitType's
Parameter variant handles only a single current-body key, while Written always
materializes with no parameters. Reconstructing Self from a runtime descriptor
or adding a Self-only materializer would violate the common source pipeline.

Extend ExplicitType with a retained frame template: actual owner definition,
full symbolic term, existing WrittenType tree. Preparation first preserves the
existing closed conversion; open conversion uses written_terms::written_template
with the actual owning body. Reconstruct its symbolic source using that owner's
actual keys and require equality to the original term: unresolved/foreign/erased
parts cannot become inference holes or guessed caller ordinals. The original
source term remains complete. The run-time descriptor uses the existing
WrittenType::materialize_with and that exact frame's current GenericEnv bindings;
wrong owner/missing binding remains a real source application error. No parser,
new solver/unifier, descriptor-derived source identity or per-entry source plan.

One cold source application helper is shared by Q51 qualifiers and ordinary
explicit argument aliases. A direct source parameter keeps its existing route;
closed applications keep their existing representation. An actual alias/Self
application enters this same finalizer regardless of syntax/macro origin.
Domain advances and retained dependencies must remain stable/explicit. Existing
cold source plans guard declaration/schema/edition/epoch/alias facts. Per runtime
use is only descriptor binding/interning over retained structure; no namespace
scan, body clone, source lookup or reparsing.

Prerequisites remain explicit: compound handwritten explicit arguments and let
annotations use the same let_annotation_thunk::select leaf mechanism, whose
PreparedTypePlan currently lacks generic parameter/alias application facts.
This slice must not invent a separate explicit-argument AST walker to bypass it.
Preserve that held shared annotation leaf/owner-hint integration as next work.
Full Const/Region packets, projection inference, generic initializer provider/
cache identity and upstream solver integration stay open. A source Template
component is not completion of those features or new stock runtime acceptance.

Evidence: extend the unchanged current Self/closed alias source controls across
all editions and original/emit/wrapper outputs; inspect actual Q51 terms and
retained template ownership, memo/withdrawal/no-executable-callee behavior. Keep
whole programs and failures honest. Recheck all eight recurring modes and
MA002/MA003/MA006 separately against an exact source fingerprint. Existing
legacy source/header/owner reparsing routes remain held; do not extend them.

V417 source review finds a guard prerequisite before integration: a new alias
explicit argument cannot return an empty per-selection guard just because its
parent query recorded the reads. PreparedExplicitTypeArguments retains a joined
runtime guard assembled from each leaf; alias/schema dependencies must enter
that guard too. Do not rely on its caller's unrelated source guard or memo
invalidation alone. Extend the one shared source application finalizer into
PreparedBodyTypeApplication(actual Entry Pc), using the existing pure query
framework and fresh append-only tag73. It retains actual SourceLocalPlan/site,
ordinary NamedTypeOutcome and final ExplicitType application (plus existing
trait-qualifier metadata), including normalization/schema/currentness dependencies.
Q51 delegates its named application to this query; ordinary explicit aliases/
parameters retain its actual PreparedInputGuard. Existing concrete Type-plan
leaves and compound annotation service stay intact. No second parser/type walk,
new trait solver or context-only name/method semantics. This source query is a
representation/admission result, not selected impl or execution permission.
V417 observations remain held at their own fingerprint until this guard join.

V418 follow-up source review: consulting Q73 and then re-entering PreparedTypePlan
for an already selected concrete leaf repeats the same named-Type preparation.
Do not keep that staged fallback. Extract the existing exact indexed Type-use
location check from EnteredTypeGoal::source_for into AdmittedBodyContext, shared
by the old goal and the new application's source_for. All named explicit leaves
then consume the one Q73 answer/guard. A missing application remains unsupported,
not `_` and not permission to infer away a written alias. Compound syntax still
uses the existing annotation selection service. Add an unchanged upstream
fn-item-type-coerce source witness: IntMap is a real function-pointer alias at
eq::<IntMap>; Q47 must keep its Q73 guard and exact source application, cold
callee bodies, memo identity and withdrawal. No runtime full-fixture claim follows.

V419 evidence keeps a new whole-program/source failure visible. Both builds
pass;4/5 selected controls pass. The unchanged IntMap whole main fails all4editions
at1dispatch at its ordinary let annotation: EnteredTypeGoal supports only
primitive aliases. Its source Q47 control fails earlier at eq's missing admitted
call signature, so it remains enabled and held; no alias guard acceptance is
invented. Next join the ordinary annotation leaf service to the same Q73 source
application (Selected and already closed Written cases), keeping actual source
site and joined guard checks. Open frame parameter annotations still require
owner-aware hint transport and must remain pending rather than reuse construction
ordinals. Preserve the call-admission diagnostic independently; do not fabricate
an eq signature or strip macro contexts to force its source control green.

V420 both builds/4of5 controls pass. IntMap original now passes its alias let
annotation and reaches the actual eq call at12dispatches in all4editions; the
next failure is source function-pointer versus function-item coercion at that
ordinary call. No whole-main pass. Its source control records ModuleCompletion,
so V421 uses the existing actual completed scoped Value discovery (factored
from prepared_impl_header_world) instead of the global impl-catalog completion.
This supplies the real call namespace prerequisite without fabricating a seal,
signature or namespace answer. All existing helper callers keep the same real
preparation. Preserve V419/V420 failed controls and runtime captures. Open
parameter annotation hints and actual function-item coercion remain held.

# Shared pattern name admission for conditional patterns

LOCAL-024 / WT03, T024/T050/T051/M014; October 15, 2026. Root owns
syntax/condition_ranges.rs, syntax/scalar_pattern_binding.rs,
frontend/scalar_pattern_binding.rs and narrow syntax/pattern_plan.rs /
syntax/literal_match.rs joins. These changes follow frozen common-source-types-
patterns-v1 (SOURCE SHA5ab61ed010a38fa14f35cc6fb37d6928cea930d440fdbef54434a1391c0f68c4)
and are excluded from that asynchronous validation.

Invariant: a syntactic IdentPat is not proof of a binding. The same actual Value
namespace authority must disambiguate reached match and conditional patterns
before a binding is staged or a conditional success is asserted. Read-only review
found condition_ranges::plan accepting a bare name as irrefutable while
PatternSourcePlan::requires_binding_admission only covered Match. Minimal source
counterexample is `const C:i32=1; fn main(){if let C=2_i32{ /* must stay cold */ }}`.
This is a source-derived witness; no guest execution has yet reproduced it.

Extend the existing ScalarPatternSite source selector to actual direct condition
IdentPat ownership, retaining real pattern NodeId separately from the LetExpr
phase owner. Both source kinds join the same ScalarPatternBindingAdmission,
actual lexical/module/imported Value namespace and actual compiler prelude proof.
PatternSourcePlan's existing pure memo retains source structure; reached Entry
and Commit obtain the same proof and condition lowering retains its dependencies
through the existing binding_draft/verifier/namespace-witness route. No constant
name list, casing rule, macro identity, extra evaluator or runtime layout proof.
Wildcard/scalar comparisons stay on their existing common tests/transaction path.

This is an admission prerequisite, not complete name-pattern semantics. Existing
proof service's original-source/current-seal limits and generated/retained Value
origin join remain missing shared infrastructure; do not weaken those checks or
claim an unsupported refusal is a runtime pass. The implementation must not create
a new generated-only branch or exempt this valid runtime case from inventory.
Positive constant-name selection, source-selected const preparation/comparison,
generated/retained binding authority and common product lowering remain required.
The broader SourceLocalIndex/LocalScopes join is separate; candidates cannot grant
binding authority. Existing namespace census and proof preparation costs are held
debt, not acceptable final hot-path architecture.

Preparation is bounded exact indexed source attachment, with existing namespace
and currentness proof cost; no body thunk or cold result is demanded. Installed
runtime operations remain unchanged. Exact source/index/name/range, source epoch,
namespace and prelude dependencies must survive Entry and Commit. Missing proof
must prevent staging/publishing a binding. Existing unchanged upstream
for-loop-while/label_break_value.rs remains the positive match binding witness;
primary conditional name witness is stock binding/if-let.rs (None branch), and
const_in_pattern/cyclic-const-dag-pass.rs supplies a runtime constant-name case.
Copy originals unchanged with provenance before adding a control that needs them;
put authority probes outside originals. New controls must exercise actual query
and namespace guard routes, not merely inspect a helper or expect any error.

Review all eight recurring modes against exact successor fingerprint before
handoff; all MA001-MA008 remain open. Tests run only in the sole validator on an
immutable combined successor; source formatting is not acceptance.

Source implementation now routes direct match/condition names through the same
ScalarPatternSite and ScalarPatternBindingAdmission. The site's pattern identity
is actual Pattern NodeId, independent of condition LetExpr phase ownership.
Conditional Entry/Commit demand proof through PatternSourcePlan, lower with its
exact source/name/index check and attach the same dependency-stamped binding_draft
plus existing namespace witness. No installed VM operation changes. Mutable
move-binding syntax uses the same lookup; ref/@ projection shapes retain existing
unsupported admission boundaries. Pure test adapters cannot stage names without
a proof. Root removed no namespace/currentness/origin checks.

Two external controls use newly copied unchanged stock source: the 2024
ast-lowering-does-not-wrap-let-chains.rs source yields exact x/y condition sites,
proofs and dependency-stamped Commit recipes; if-let.rs's actual None clause
requires exact PATTERN-PRELUDE refusal at Entry and Commit through the query path
in all four owning editions. Neither control executes or passes the whole stock
program. Original files/licenses/directives remain unchanged; cyclic-const-dag-
pass.rs is also imported, still unexecuted, for the required constant-name runtime
continuation. A returned binding recipe fails the negative control; only the same
exact typed diagnostic across standard query error representations is admitted.

Eight-mode separate source review: (1) no assertion/expression evaluator;
(2) no output executor or macro identity; (3) no method/numeric policy;
(4) no new source-origin branch, but the existing original-only proof service
remains a shared generated/retained Value-authority prerequisite, not acceptance;
(5) common source proof/transaction, no cold arm body lowering; (6) existing pure
plan remains memoized, direct bounded source attachment selects the name, existing
namespace remint cost stays debt; (7) existing full namespace census preserved for
safety, not claimed final dispatch design; (8) shared Arc/index and direct range
attachment, no new immutable deep copy/full-file scan. All MA001-MA008 remain open.
No compile/runtime/milestone result exists for these successor bytes.

Exact five-leaf source review fingerprint: `2e3db2d8e92f5e5d9deb8a91bacee71ed96cc2ccb59431bab83d3c2645d23fcb`. Encoding SHA256 of sorted `sha256 + two spaces + relativepath + LF` rows. Sources copied to local immutable `artifacts/macos-first/common-condition-name-admission-v1`. Upstream import manifest/three originals must accompany a combined validation; these five are only the admission code/control slice.

## General occurrence proof before product integration

The pattern peer requires the same name proof at any actual reached IdentPat
inside tuple/or/@ patterns. Root extends source selection through indexed pattern
ancestry to the actual owning MatchArm or LetExpr, preserving lexical boundaries.
Retained PatternBindings.group_for establishes occurrence membership and uniform
canonical candidate identity/context/mode; it never establishes Value absence.
ScalarPatternSite and the opaque current proof retain both reached occurrence and
canonical cell identity. Namespace lookup still follows actual source name range
and source-order visibility; match the existing LocalBindingRecipe by exact
name_range, not by assuming its whole root pattern range equals the nested leaf.
No token casing, value/layout inference or alternate pattern evaluator is allowed.
Product lowering consumes the proof's canonical ID; the existing scalar route's
actual occurrence==canonical behavior stays checked. Generated/retained Value proof
origin and constant execution remain required integration work.

Read-only review found a required common prerequisite: SourceLocalIndex rejects
repeated explicit bindings in separate or alternatives before any demanded-site
lookup, because all share an arm scope/visibility/name. Root will extend that
same builder with optional actual prepared BodyIndex binding equivalence (no new
resolver): validate exact source body/profile and source-name ranges, map each
occurrence to the canonical existing LocalBindingRecipe identity only when the
retained hygienic group has no conflicts, and insert one local row per equivalent
candidate. Resolution inputs for equivalent occurrences must agree. Missing or
conflicting equivalence preserves typed refusal. Do not collapse duplicates by
spelling or relax non-or conjunction/mode/context conflicts. Default callers with
no prepared pattern evidence retain their existing conservative behavior; the
reached shared pattern proof supplies the actual index to the same builder.
This removes the whole-body false duplicate rejection for actual equivalent
or-patterns, rather than changing the selected witness. Build bounded range/id
maps once per existing local-index preparation; no per-operation namespace work,
new parser, Cartesian expansion or runtime evaluator is added. The existing pure
local-index preparation/revalidation cost remains separately held debt.

Prepared-index equivalence is implemented in the same SourceLocalIndex builder with private helper `frontend/source_local_patterns.rs`. It checks exact source handle/profile, indexed name attachment, retained hygienic canonical group and actual local recipe name/scope/visibility/form; maps each equivalent occurrence to one actual source identity and preserves conflicts/default no-evidence refusal. It adds only bounded temporary body-local maps. Group occurrence lookup is binary over the already sorted rows; no per-binding scan of all equivalent alternatives. Cached relevant bucket proof still must replace existing global-census remint before product leaves/retries consume this service. V3 compiled all these leaves and controls; admission tests have not yet executed. Eight-mode review: same source/name builder, no evaluator/macro/type policy, actual source authority unchanged, no cold executable demand, bounded body-index/map preparation, existing namespace costs held, no immutable AST/index copies. Exact V3 source hashes:
- `crates/lrr/src/frontend/scalar_pattern_binding.rs`: `25eef17eb5b5c517756aa877c3f049c424af47e6e469b122160d5153841bd4ee`
- `crates/lrr/src/frontend/source_local_patterns.rs`: `cdbd9c60807dbfe9052d2656a8d35a8a15cc47a672c573824f38e878b0f7ac3e`
- `crates/lrr/src/frontend/source_locals.rs`: `9e9cc1da6e8bf73d2129d810febf5bf35d0fb78b7cc18e9442db906513faf287`
- `crates/lrr/src/syntax/pattern_bindings.rs`: `ecfa2f2ecdd489247637bed79949a2b694930601296050fe9d1e7a5716f6bf33`
- `crates/lrr/src/syntax/scalar_pattern_binding.rs`: `2bad9d425fa902dd3d129e8eb96bf87c7de79c1744d02fc311660ec7aef2cc11`

Actual independent admission terminal811be0/1 recorded6pass2fail/8: the unchanged
nested or/@ authority control passed, while conditional x/y and None controls
failed before namespace/prelude lookup because entered_macro_facts has no LetExpr
binding collection. Preserve those exact failures. Root extends the existing
source lexical-facts service with conditional scopes and actual clause binding
recipes. Share only top-level && clause source decomposition with conditions.rs;
executable grammar/type admission stays lazy and existing condition-plan checks
remain at the reached demand. Each condition/consequent scope spans the actual
condition start through consequent/body end, excludes else and later statements,
and each binding becomes visible after its own LetExpr (not in its scrutinee).
Use the same BindingBudget and pattern producer recording, with an explicit new
stable Condition binding-form tag. Source discovery collects structural names,
not guest values or executable leaves. Parent scope visibility and bounded
source traversal/allocation remain checked. Later clauses/body may see earlier
successful bindings; else/after-loop may not. This is shared source infrastructure
for handwritten/generated syntax, not condition-specific runtime semantics.

Condition lexical facts source-only successor is implemented, formatted and frozen independently. Source discovery and ordinary condition plans share structural && decomposition; the former does not perform misplaced-let/executable/type admission. New scope excludes else/after, clauses expose names only after their own source range. Exact changed source:
- `crates/lrr/src/syntax/conditions.rs`: `643a910c666af5adc8eee6b522c696fe7ee6b9c46364e1d4521b9b8fc7a47495`
- `crates/lrr/src/syntax/local_bindings.rs`: `38b25fe3d063fa504c493c7632332fe51456a09b8ea5ed62ee21e103f77b030f`
- `crates/lrr/src/syntax/entered_macros.rs`: `fe9dc72ea2a5f400eca6cdcf455629d83165d9e0c15a64ca9c7180e686eb305d`

Eight-mode source review: no evaluator or macro identity; no method/numeric rule; same source lexical service for actual parsed contexts; no guest effect/cold executable preparation; bounded one-time structural source walk/maps; no new namespace census; no immutable index clone or complete-file node lookup. Existing namespace remint and generated Value origin remain required common prerequisites. This is unexecuted source; earlier6pass2fail receipt is preserved, not revised.

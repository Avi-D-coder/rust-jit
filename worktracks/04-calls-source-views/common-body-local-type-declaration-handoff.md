# Common body-local Type declaration successor — source only

Pre-code bounded record, 2026-10-15. T020/T calls-source-views source Type
prerequisite; revision5/E-R5-005 shared source constraint obligation. Parent
owns private current environment/declaration publication and memo accounting;
worker owns body_type_declarations.rs only for this successor, controls outside
upstream originals. Frozen common-body-type-declaration-v1 source4 is historical
and unchanged in its copied overlay.

Invariant/minimal witness: an actual opaque local Type prefix selects one actual
body-local declaration. Its canonical source identity comes from that admitted
owning body's exact facts and source, equally for original, retained and generated
syntax. An expansion's module definitions map does not own declarations inside a
function body. Unchanged borrowck-assign-to-subfield.rs A/B local types are the
primary witness; full stock runtime remains unexecuted. Existing generated/wrapper
source controls do not count as full stock or local constructor execution.

Shared entry: PreparedSourceLocalPlan::prepare_body_type -> opaque type_prefix ->
body_type_declarations::prepare -> source_types::prepare_body_declaration -> the
single prepare_nominal kernel -> actual TypeSchema. Local lookup order, declaration
NameRef class/edition, exact AST/header tokens, namespace, selected id/unit/scope,
source currentness and relevant dependency stamps remain unchanged. The obsolete
canonical branch order (GeneratedSyntax/RetainedSyntax before local owning facts)
is replaced for actual local prefix selections only. ModuleTail retains its prior
canonical module map/source publication path; there is no fallback resolver.

Authority: root's genuine publication supplies exact DefinitionSource/Header/
DeclarationContext rows for the actual facts item; caller raw schema/name/id is
never accepted. The local branch must find the selected item in the same retained
context facts and match parse/header/range/scope exactly before canonical DefKey
use. Missing publication or facts stays typed. Generated item maps and rewritten
original DefKeys are not substitutes. Private current owning-environment query
publication remains prerequisite before production use/constructor install.

Costs: cold preparation selects the retained local row (existing bounded facts
scan), then retains existing parser/index/plan roots. No new cache or per-dispatch
planning; ordinary installed execution has no new source work. No BodyIndex/map
copy. Exact Node/range lookup and existing cached TypeSchema stay common. Root's
selected header publisher must remain coordinator-only and atomic; memo/owner-root
registration stays parent-owned. Eight-mode review and exact hashes follow after
implementation. No Cargo/native validation by worker.

## Implemented API and removal

No new production API. `body_type_declarations::prepare` now takes the existing
opaque local selection through exact current `context.facts().items` first. The
same canonical local DefKey/header/range/scope/ParseKey check is used for every
source origin. ModuleTail preserves GeneratedSyntax module map / actual original
Outline / explicit retained module declaration hold. The former source-origin
classification preceding the local branch is removed; no generated-only second
resolver or relaxed member identity is introduced. `source_types::prepare_nominal`
and the entire final exact declaration Name/edition/context/header/schema/current
check sequence are unchanged from frozen source4.

New tests (uncompiled/unexecuted):

- `unchanged_local_types_join_their_actual_published_body_declarations` uses the
  complete unchanged borrowck A/B fixture in all four editions. Genuine opaque
  local prefix first requires exact missing publication; coordinator's actual
  `publish_local_declaration` then admits selected header/source/context; shared
  Type proof must preserve actual canonical DefKey/nominal identity. No Thunk,
  Constant or FunctionSignature preparation; epoch invalidation remains refusal.
- `generated_and_wrapper_local_types_use_their_actual_owning_body_facts` emits
  that same complete unchanged original as actual arbitrary macro and wrapper
  output, obtains actual discovery/GeneratedSyntax/body boundary, then uses the
  identical local publication+Type proof helper. All unexpected discovery steps
  remain typed panics. These tests are requests for validation, not passes.
- All previous source4 controls remain intact, including the negative local
  publication prerequisite. No retained-body positive control was invented;
  actual retained source/body bridge is still a distinct unexecuted matrix entry.

Root dependency is immutable `common-selected-local-declaration-publication-v1`
COMPONENT SHA9209723121b5354cffd1ce9c20a08b7d64ac218f514d8501bce4d61af2b8f8ec:
coordinator-only exact selected facts/AST publication, memo module entry, TypeSchema
exact syntax lookup. Source-only review of its actual API found coordinator/run
refusals, current context check, unique facts row, attribute producer hold,
source/profile/unit/module containment, actual header tokens, exact row equality,
collision/budget/revision preflight before publication. No production/source name
scope/body grant. Its acceptance remains owning validator's actual report.

## Exact eight-mode source review

Inspected against successor fingerprint recorded below, not a feature closure.
MA audit remains distinct from recurring modes; E-R5-005 common source constraints
and MA005/MA006/MA007 source identity/preparation/authority concerns remain open at
the broader architecture gate.

1. No assertion/expression compiler or evaluator: changed only declaration
   canonical authority branch and external source proof controls. Runtime route
   and obsolete assertion-route removal are outside this component.
2. No literal/break output executor: real GeneratedItems/current body boundary
   enters same local prefix/facts/schema route; output body is not executed here.
3. No context method/numeric rule: no runtime descriptor, arithmetic or selected
   method behavior changed; TypeSchema/source proof kernel stays single.
4. Source-origin refusal removed at exact local facts join, preserving all current
   source/header/range/scope/Name/hygiene/schema checks. Module retained publication,
   alias/import/opaque pinned adjustment and actual current-environment publication
   remain genuine typed prerequisites, not accepted cases.
5. No new pattern machinery/eager arms: facts are existing owning lexical facts;
   no executable body/thunk/arm is prepared. Test checks forbidden query demand.
6. Cold O(local facts) selection in already retained plan, same cached ParseFile/
   TypeSchema, no new query framework. No installed execution caller added.
   Parent must cache genuine current environment before production switch.
7. No NameNamespaces construction/census. Relevant source/header/declaration and
   actual module semantic dependencies remain inherited; local current owning
   environment authority remains parent integration prerequisite.
8. No parser/BodyIndex/map deep copy or whole-file search. Existing exact syntax_at
   joins remain. New two branches simply reorder retained facts vs module maps.
   Root's generic db-fetched Outline weak pin scheme already charges actual inner
   Arc after memo loss; no second registry added. Plan exclusive roots/current
   publication accounting is separately unexecuted. Prior cold TypeSchema wholefile
   scan was repaired in root's dependency, not credited to this worker.

Runtime/stock fullmain/Box/std macro, retained-local, suspension/cleanup/exact-once
and production constructor entry remain unexecuted/held. A Type proof component
control is not a language fixture execution pass. Parent owns integration/validation.

## Reviewed exact source

Fingerprint `88cc4cdb09f0bae2e6559def2d46b47dd21ddf6f981658bfd29aa102e7f011da` (compact sorted-key ordered JSON rows SHA256).

- `crates/lrr/src/frontend/body_type_declarations.rs` 22827 bytes SHA256 `33ae1c3fed6a42ac7de80a3ecc2737c64593106b704bc03efa6c690c0b05aa2f`.
- `crates/lrr/src/frontend/body_type_declaration_tests.rs` 20345 bytes SHA256 `bb3562e735fe61c33f9d07286021b10a4318478b510391bac2e4d41bf1d7d72d`.

No Cargo/native test or Rust compiler was run by this worker.

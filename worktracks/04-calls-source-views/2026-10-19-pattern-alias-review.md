# M018/T043 pattern alias source selection — separate source review

Solo LOCAL-049. Parent78ab2fed1f61f55ea856aeb7820d338b850e5662. Reviewed
V403 fingerprint `5a40ec47f2986ec904cd7f3de4973b99a8d3c751abface41d65e19b7c1aa5e51`.
Validation is pending at this review. No task/audit/runtime gate closes here.
Applicable audit findings are MA002 (pattern semantics split across contexts),
MA003 (numeric inference selected by assertion identity) and MA006 (methods
whitelisted inside the assertion evaluator). The common selection/constraint
route must not extend these alternate semantics; these audit IDs are distinct
from the eight recurring modes below.

Actual route: PatternTree::Builder::path_slot retains the real indexed
path-pattern owner. SourceConstraintPlan::build indexes node->retained tree/
slot and classify uses the existing capture_path_arguments for actual generic
Type nodes. QueryKey::SourcePatternName(tag72) reads that canonical row and
SourceLocalPlan, then select_source_pattern_name_inner uses EnteredValue or
the common module selector. finish_pattern_declaration sends aliases through
prepare_selected_definition_with_arguments/AliasNormalizer and existing mixed
Substitution. Exact nominal arguments remain metadata, not matcher behavior.

select_source_pattern_name is shared by pattern_access and
binding_constraint_nominal::pattern_constructor; the latter's separate lexical
EnteredValue branch is removed. Its existing constrain_term/project_term
propagates a selected alias application's facts through existing component
projections. Runtime resolve_pattern_name uses the same query and ordinary
pattern_selection_target/pattern_constructor/field_ordinals. The transactional
matcher, VM operations, scrutinee and cleanup ownership do not change.

1. No assertion/expression compiler or evaluator. No macro name, assertion
   operand or synthetic executable source is introduced.
2. No generated literal/break path. Original and emitted/wrapped source have
   actual node/edition/epoch ownership. Synthetic Option language-item slots
   explicitly have no source node and keep their previous library lookup.
   This is not evidence of full standard-macro/runtime matrix closure.
3. No method whitelist or numeric rule selected by context. Alias application
   metadata enters existing source constraints; there is no new unifier,
   candidate algorithm, trait solver or actual-type method fallback. Upstream
   solver/member/VM integration and four required solver source clients remain
   open independently of this repair.
4. No original-source-only refusal or fake node. All genuine path patterns use
   the same source query. Missing alias arguments remain explicit unsupported
   inference, for handwritten and generated code alike. Enum-alias variant
   and partial/symbolic application inference remain held gaps.
5. No second matcher, pattern retry or executable-arm validation. Existing
   capture/access and numeric/nominal services consume one selection. Name
   metadata preparation does not execute a constant or lower a callee/result
   body. Cold refused alternatives retain their prior reached-only failure.
6. Metadata indexes build once in the existing pure memo. Q72 stores only
   key/name/selected identity/application, no parser/body graph. Runtime cache
   hits check retained PreparedInputGuard stamps and do not fetch or parse.
   Failed query results cannot mint a guard: Refused targets from errors now
   remain attempt-local, not cached across source publication. Unsupported
   cold-name attempts may repeat metadata queries; dependency-aware typed
   pending results remain a documented follow-up, not a normal-path claim.
7. No full-world map or comparison added to installed dispatch. Q72 cold
   preparation uses existing source/environment/name services and demands
   TypeSchema so constructor declaration changes invalidate its answer. Its
   exact successful memo supplies the runtime guard. Existing synthetic/
   identifier and constructor binding/name/schema routes remain migration debt;
   this review does not claim all existing field-binding dependencies repaired.
8. No BodyIndex/parser deep clone or complete-file node search. A BTreeMap
   locates a retained tree/slot in O(log N); canonical names are small metadata
   clones, and immutable parser/body/tree handles stay shared. Arc::make_mut
   may clone only the small Constructed schema summary during cold constraint
   preparation; it does not clone a body/index. Stable domains advance for
   added name/site metadata and SourceConstraints schema advances14->15.

No new cap, accounting, unsafe, IO, macro execution, static compiler check,
query framework or backend. No performance measurement or speed claim. Cache
hits still validate relevant guards after input revision changes; their cost
is proportional to captured relevant inputs, not namespace population.

Evidence required: exact macOS1.99.0 build terminals, unchanged source/HEAD/
binary guards; actual upstream pattern names in all four editions and three
source origins; pointer-reused memos, source withdrawal and no guest planning;
related pattern/nominal/invalidation/cold/exact-once controls; actual unchanged
alias main captures. Unsupplied alias inference and whole runtime/full-fixture/
standard-macro matrix, milestone and JIT gates stay open even if host checks
pass. Later source edits invalidate the relevant conclusions above.

V404 source review follow-up (fingerprint
`da62da167e42be0c006bdfeba886fc3fbdf555959f6c3ea41086e099fdaea478`):
V4039/14controls exposed the canonical module receipt's Value-only prerequisite;
0/8targeted alias runtime rows passed and no regression was accepted. The common
canonical_module_declaration_in_namespace now validates the actual namespace
selected by EnteredValueGoal and uses the corresponding original candidate/
generated declared-name facet. The canonical_module_declaration wrapper remains
Value-only for prepare_entered_module_function and its rejection controls.
This extends the shared declaration metadata projection, not a pattern-only
kind guess or call-authority bypass. Actual source/header/context/owner/edition
checks and retained/generated attachment proofs remain required.

The unchanged nominal-owner control source now enters the full coordinator via
a cfg(test)-only completed-report helper instead of the bare opcode harness,
which has no owning source environment. Actual faults/panics remain failures;
there is no runtime implementation in this helper. The other bare opcode
controls remain separate. No new body preparation, executable alternate route,
namespace dispatch scan or immutable body deep copy is introduced. The stale
annotation doc comment is moved to its actual Type-shape accessor. Remaining
review conclusions and holds above still apply; V404 terminals pending.

V404 runtime observation while the separate control bank runs: unchanged
struct-aliases.rs reaches247VMdispatches in all four editions (parent21),
completing the first S2 construction/match/assertions before the next S4
construction reaches the explicit inferred-application hold. These are still
four failed/incomplete captures, with no observed cleanup, not four stock
passes or a complete fixture. Static-method aliases remain at9dispatches/
1source call and the lexical-callee hold. The exact runtime runner is retained
as 2026-10-19-alias-runtime.py, byte-identical to the executed temporary script;
its hash and reproduction commands belong in the evidence receipt.

V405 source review follow-up (fingerprint
`00ba39daeff77500f520bfd89ccded384a09983dfe80d470735e33d4173ba5af`):
V40414/15controls passed. The exact source witness caught a missing shared
metadata dispatch: SourceConstraintPlan::build ran classify_pattern for binding
rows but skipped classify for all pattern nodes. The same classify now also
visits real indexed patterns, retaining their path argument Type sites through
the already shared capture_path_arguments. No parser, type heuristic or
standalone pattern argument implementation is added. Names/parameter rows
remain the pattern engine's metadata; no executable arms/bodies are prepared.
V404 runtime partial progress remains evidence for its own fingerprint only.
All other review conclusions and open gates still apply; V405 terminals pending.

V405 terminal evidence: both builds and15/15selected controls pass on the
reviewed fingerprint, with source/HEAD/binaries unchanged through final guard.
The actual source pattern matrix retains24applications/12edition-origin worlds
and12explicit inference holds, warm memo identity and source-withdrawal guard
revocation. All8targeted alias runtime rows still fail; no new stock or full
fixture pass. See worktracks/evidence/2026-10-19-v402-v405-pattern-alias-applications.json.
No architecture/task/solver/milestone/JIT gate is closed.

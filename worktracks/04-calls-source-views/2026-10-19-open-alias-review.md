# M018/T040 retained open applications — separate source review

Solo LOCAL-049. Parent4b525bc40a671cffb757aa4fae78d4376ca8ca2a.
V406 candidate fingerprint
`047646536d1ba4e083ca79d30d7d0d958aa6c4ed9f0362a0ad09059d3fbd617b`.
Actual audit constraints: MA002 split pattern machinery; MA003 assertion-selected
numeric inference; MA006 assertion method whitelist. None is closed here.
Validation is still running at this source-review point.

Actual route: source_types::prepare_alias_application reads TypeAlias's actual
generics, completes missing Type slots with that declaration's real ParamKeys,
and calls the same prepare_selected_definition_with_arguments finalizer.
AliasNormalizer/retain_alias_type/deferred_alias_type/resolve_actual_type and
Substitution retain the actual target Type and substitute once. No descriptor,
callee body, new parsing, trait selection or inference occurs in this adapter.
Missing const/default inputs remain unsupported; supplied actual Types retain
prior behavior. Omitted regions retain the existing actual erased-source rule.

SourceTypeApplication stores the whole target and unresolved declared keys.
constructors::bind_prepared_record_inner retains it separately from runtime
explicit arguments. An open target binds its ordinary generic schema; the alias
Params are never installed as caller-frame Params. PreparedRecordConstructor
retains this metadata (domainv7). PatternSelection retains the same metadata
from the same helper (Q72 value domainv2). Alias spelling selects no rule.

binding_constraint_nominal::record_constructor attaches that same application
to the small Constructed schema summary. application_relations projects it
through the existing project_parametric/parameter_paths/Component work queues;
concrete projections constrain normally and repeated actual keys connect their
source positions. OpenShape takes the complete retained target/keys. Existing
solve_shape/solve_projection/component relations answer the variables, and the
ordinary prepare_open_expression decorates the ordinary record recipe with its
result hint. Its existing dependencies and guards own source currentness.

Installed run_constructors preparation reads the retained open-application flag
and actual result/source context from the already installed hint or reached
parent. It refuses an unsolved application instead of executing target holes
and losing the alias's fixed/repeated relationships. No source query/planning is
added to execution. VM pattern matching and Frame record construction remain
unchanged. Their normal descriptors, mapping, hints, temporaries and cleanup
own execution. Legacy unsealed record/generated binders remain held debt.

1. No assertion/expression parser, compiler or evaluator. Actual standard
   assertions in the unchanged main execute through their existing ordinary
   expansion/lowering route; they do not select a numeric or alias rule.
2. No scalar-only generated route. Ordinary/user-macro/wrapper source use the
   actual source alias/generic identities. Full standard-macro/all-pattern
   origin/cleanup/suspension acceptance is not inferred from one fixture.
3. No member whitelist or macro-context numeric inference. Application metadata
   is a shared source constraint producer. Existing structural projection and
   inference machinery is reused. No upstream trait/canonical/cycle algorithm
   or custom trait fallback is added. Remaining upstream clients/root/member/
   VM integration and inherited custom solving consumers remain open.
4. No original-only refusal, fake parameter or fabricated Type/source identity.
   SourceTypeApplication's unresolved keys are actual declaration parameters;
   no target slot is inferred from alias argument position. Const/default
   missing-source applications remain explicit gaps in every origin.
5. No matcher/traversal duplication. Pattern selection and capture typing keep
   their common service. walk_constructor adds the same application relations;
   the current root_via suffix proof reconnects a genuine relative projection.
   A rewritten nested prefix with no such proof stays a Binding demand rather
   than receiving an invented prefix. This nested relation gap remains held.
   No unselected executable arm or callee body is lowered for the inference.
6. Cold metadata/constraints are pure memo predecessors, keyed by actual body/
   site/environment. Installed code reads retained results and checks runtime
   context only, with no parsing or source-plan reconstruction. Different
   source sites may request distinct application facts; installed reuse never
   reruns the source finalizer. Failed/pending source cases are not passes.
7. No full-world namespace construction/comparison in installed dispatch. Cold
   source services keep their existing relevant name/declaration dependencies;
   runtime open-context validation walks the current Type term, not namespace
   population. Existing incomplete cache/name/schema routes remain held.
8. No parser/BodyIndex clone or whole-file node search. Retained declarations
   and actual Type syntax stay shared. Only small application/schema metadata
   is cloned during cold query preparation. Source parameter order comes from
   actual ordinal metadata; all stable encodings are explicit.

No new cap, memory accounting, sandbox/static checking, unsafe, query framework
or backend. No scaling or speed claim. The actual cost of source application
preparation follows the retained alias target and parameter relationships; a
runtime open-context check is proportional to the reached Type term.

Executed evidence at review time: actual unchanged struct-aliases.rs completed
961VMdispatches with complete unit/cleanup captures in all four editions. The
other four static-method alias rows still fail at9dispatches/1call. This is one
new distinct runtime contract across four edition rows, not four new fixtures,
a refreshed suite total, diagnostic/full-fixture or task/interpreter/JIT gate.
The complete15control bank and final source/binary guard remain pending.
Unexecuted repeated/permuted/compound application matrix entries stay explicit;
source inspection of preserved terms/relations alone does not prove acceptance.

Final V406 evidence: both builds and15/15controls pass; all tested sources,
HEAD and binaries match through the final guard. Source controls verify36record
bindings/36pattern selections/12actual inferred u8,char applications across
12edition-origin worlds, plus relevant memo/withdrawal controls. Runtime gain is
1distinct original/four edition rows, with4other failed rows. Scope/holds above
remain; no task/milestone/solver/JIT closure. Receipt: worktracks/evidence/2026-10-19-v406-open-alias-applications.json.

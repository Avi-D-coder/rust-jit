# M018/T040 actual body alias normalization — source review

Solo root, LOCAL-049 and same-owner LOCAL-037/046 interfaces. Parent
35f36e738df6d7968a8c7f6c721f4119dfb8dc1c. Reviewed candidate source fingerprint
`e5415952f57082591b21f231a366e5c17fbe06a4c82becb9901b5128ef8938c9`
(V395). This is a staged source-resolution prerequisite, not runtime alias,
M018, M020, interpreter or JIT acceptance. Actual terminals are separate.

Shared route: actual coordinator publication of the main BodyHandle -> existing
SourceLocalPlan and canonical local rows -> local_declaration_owner. Type and
nominal callers retain filtered wrappers over that same source/header/body/
plan/name/edition join. AliasNormalizer accepts a genuine cold declaration or
this entered declaration receipt. Actual TypeAlias/DeclarationGenerics retained
target AST -> WrittenTypeDeclaration/resolve_actual_type remains the sole
changed alias Type walk. Mixed Substitution remains the argument owner.

The missing entered header name join now uses the existing HeaderItem metadata,
actual ParsedFile::declaration_type_name receipt and token context/edition.
PreparedSourceLocalPlan::resolve_declaration_type_name and fields/parameters
share extracted prepare_header_prefix -> SourceLocalAdapter's unchanged
lookup_prefix_inner -> declaration_index/local imports/module observation.
No module-name fallback is supplied when lexical selection is incomplete.
NamedTypePreparation's selected-definition finalizer is extracted so opaque
body prefixes never fabricate a NameDefinition with default unit/edition.

Applicable audit identifiers are MA003 and MA006 (context-independent source
constraints/member identities); neither is claimed closed. MA001/MA002/MA004/
MA005/MA007/MA008 are distinct historical findings, not these recurring numbers.

1. Separate assertion/expression evaluator: none added. Changes select real
   source Types, not executable expressions. No macro identity, assertion
   operands, synthetic function, opcode, verifier or VM evaluator changes.
2. Literal/break-only generated execution: none. Genuine original/generated
   source rows and token metadata enter the same alias/name services. The
   source control exercises arbitrary user emission and a wrapper; it does
   not establish standard-macro runtime execution or the broader matrix.
3. Context-specific methods/numeric inference: none added. No method names or
   target struct names are matched in production. Runtime method selection
   and its missing upstream solver answer join are unchanged. The test helper
   supplies one explicit u16 argument as a source-template observation only.
4. Generated exclusions: no new original-only condition. Real owning receipt
   checks replace an unconditional local alias refusal. Cold and entered source
   receipts remain distinct proof joins; matching header text or an owner bit
   grants nothing. Missing/incomplete source stays error/pending, not guessed.
5. Duplicate patterns/eager executable checks: no pattern implementation or
   static checker. Actual local owning-body metadata is prepared once by the
   coordinator; aliases and nested declaration headers never request a callee
   executable body, thunk or pattern plan. Generic target templates remain
   deferred until ordinary constraints bind their parameters. No runtime pass
   follows from a header or target template alone.
6. Repeated planning: extraction shares prefix construction; no parser or
   display-token reparse is added. Existing immutable SourceLocalPlan/parser
   handles remain Arc-shared. Header identity/generic rows are retained with
   explicit stable domain v5; prefix Name location uses explicit tag3/domain
   v6. The actual leaf receipt uses direct syntax_at on a cold demand. Legacy
   alias normalizations outside existing memos still walk retained targets on
   demand; a complete memoized alias plan is remaining debt. No installed
   dispatch preparation route or speed claim is established by these checks.
7. Namespace scans/invalidation: no census in installed operations or call
   entry added. Existing module observation and owning currentness supply
   name dependencies. Entered alias dependency traces retain actual local
   owner and owning-plan query stamps; the sole legacy traced runtime consumer
   still pins database/source revision conservatively. Complete selective
   alias memo/negative-dependency integration is not delivered here. Existing
   broader lexical_namespace_stamps callers remain named migration debt.
8. Body copies/whole-file searches: no deep BodyIndex/ParsedFile clone, file
   scan or parser is introduced. The existing owner-row join is a cold linear
   item-row lookup and direct syntax_at. Header item selection is an ordered
   map lookup; generic lookup uses the existing header list. New metadata is
   an additional admitted header kind and one actual demanded leaf, with
   checked allocation arithmetic; no fuel/time/count/byte/resource limit.

Removed affected routes: SourceLocalPlan's body-local primitive/one-token
shadow heuristic and forged NameDefinition fields. Existing normalizer/simple
alias/header consumers and legacy resolve_written_type routes elsewhere remain;
this is not their global removal. Constructors still consume BodyTypeOutcome
and hold AliasContext before normalization. Qualified local calls still lack
an entered rooted-call recipe; select_lexical_associated_call still consumes
its root declaration without carrying an alias target Self term. They remain
explicit next integration work, including S4<U> -> S3<U,char> relationships.
Do not forward alias arguments positionally, rewrite names or remove a source
receipt guard to make these runtime cases pass.

Cold costs are existing source/plan currentness, direct declaration lookup,
linear owner/generic rows, ordinary local/import/module lookup and retained
Type traversal/substitution. No new unsafe, compiler/backend/dependency swap,
macro execution/IO in a query or live rustc checking. Performance, suspension,
exact-once effects, cleanup, complete stale/wrong owner/epoch matrices and full
runtime origin equivalence require their own evidence. Target-only source
success cannot establish those unexecuted contracts.

V389-V390 preserve compile failures (old owner annotation and host Debug
formatting). V391 preserves a missing publisher test setup. V392 preserves
misuse of an impl-catalog seal for execution entry. V393 preserves the actual
StringVec local header name gap. V394 passes the 48-alias/24-world source
matrix. V395 generalizes the owner kernel to named non-Type headers while
keeping Type/nominal wrappers restricted. Its final terminals are pending
at review time; append actual outcomes before checkpointing.

## V396 shared retained target follow-up

V395 final outcomes: both builds;7/8 controls pass; all8runtime rows fail at
the existing lexical-callee/constructor guards (9VMdispatches/1call and
2VMdispatches/0calls respectively). Exact source/head/binary guards pass.
The failed explicit-Type control is preserved, not removed or weakened.

V396 source fingerprint
`539e1117d7f7fc022a1341776e5e9554b9589f0a487d9fbc81be64308d451b3d`
retains the same parent. retain_alias_type now owns the previously duplicated
actual TypeAlias/DeclarationGenerics/header/context/target AST join. Compound
target substitution and normalizer simple-name chains share that retained
query/parser handle. The simple-name branch uses the actual target NameRef
with resolve_declaration_name_at; its former occurrence-free token lookup is
removed. normalize_traced still owns cycle detection and chain traversal;
no recursive shortcut evaluator or primitive shadow rule is restored.

All eight modes were rechecked on retain_alias_type and both changed consumers.
Only cold declaration-source joins change; no macro identity, new execution
path, generated exclusion, pattern or compiler static check, installed source
planning, global hot-path scan, body deep copy or whole-file AST search.
The already existing conservative traced return revision guard remains named
debt; this helper does not claim to supply complete selective dependency proof.
The source review and observations of V395 remain attached to that fingerprint;
actual V396 terminal acceptance must be recorded separately.

Source review also found an obsolete xtask expectation named
alias_depth_budget_rejects_before_fetching_the_unprocessed_declaration. It
expects the removed LRR256 alias-depth budget and conflicts with E-R5-006.
It is not selected or counted as passing in this bounded run; do not restore
a production cap to satisfy it. Its cleanup remains explicit maintenance debt.
The separate chain/cycle host controls are selected for the affected semantics;
this does not claim the entire type_aliases integration target passes.

## V397 identity-strengthened checkpoint review

V396 both builds,8/8 exact library controls and3/3 selected existing xtask
chain/cycle/pending controls pass. All8runtime rows fail at the same disclosed
lexical-call/constructor guards, with incomplete captures and no observed
cleanup; no parent runtime comparison or new stock gain is asserted.

Reviewed final candidate source fingerprint
`3d39336fd674bc98628391eb945a256263fd1ac3774e6c6e6de96210cd95954c`
(V397), same parent. retain_alias_type additionally compares both actual alias
and generic-declaration definition identities against the demanded key. The
unused ast::self import from the extraction is removed. The control now compares
actual Qux, Vec, String, S and S3 declaration identities from ordinary target
source lookup, rather than accepting any nominal with the same argument shape.
All eight source-path conclusions above still apply; the two exact key checks
are constant work and create no source-name semantic special case. Final build/
control/runtime evidence remains pending until actual terminals are collected.

Final V397 terminals: library build session58092/exit0, CLI build18987/exit0,
xtask alias build45838/exit0; library controls69937/exit0 (8/8), selected existing
alias controls69016/exit0 (3/3), runtime observer48949/exit0 with8guest failures.
Source/HEAD/test/CLI guards match before/build/after-runtime at the reviewed
fingerprint. The source alias matrix checks48aliases in24worlds, including
exact target identities and revocation. Existing region/field/header/cold-Type
controls pass. Both unchanged rustc alias mains remain runtime failures in
all4editions; no refreshed suite count or new distinct stock gain. Raw local
logs are disposable; compact tracked receipts and exact invocation/provenance
are in worktracks/evidence/2026-10-19-v389-v397-body-alias-normalization.json.
No complete fixture/task/milestone/solver/interpreter/JIT/performance claim.

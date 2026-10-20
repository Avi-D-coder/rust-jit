# M018/T043 — source pattern names and alias applications

Solo root LOCAL-049 with same-owner LOCAL-037/044/046 interfaces. Baseline
78ab2fed1f61f55ea856aeb7820d338b850e5662. Unchanged primary witness:
structs-enums/struct-aliases.rs, currently stops at shared S2 pattern-name
classification after21VMdispatches in every edition. No new agent/claim.

Invariant: one source pattern-name service selects the declaration and its
actual alias target application for runtime matching, capture typing and source
constraints. The transactional matcher does not acquire alias or macro rules.
S4::<u8> must retain actual indexed arguments and yield S3<u8,char>; display
spelling cannot be reparsed or substituted into another source definition.

Existing route: PatternTree::Builder path_slot -> PatternName -> ordinary
lexical EnteredValue or module lookup -> pattern_declaration -> ordinary
NominalConstructorTarget/schema/field binder -> existing PatternTarget matcher.
Capture typing uses select_source_pattern_name, but binding_constraint_nominal
currently duplicates lexical selection and bypasses that shared route. Repair
that duplication. Preserve the existing pattern/continuation/cleanup owners.

Prerequisite: retain each real path pattern's actual indexed owner NodeId in
PatternName. SourceConstraints captures its path generic arguments through the
same capture_path_arguments used for expressions. Index pattern names by their
actual node to retained tree/slot, not copied parser/body graphs or whole-file
searches. Expose stable name metadata encoding and advance relevant domains.
The original type argument selector then handles each genuine Type occurrence.

Add a pure source pattern-name query (tag72) over that actual node/body. It
reuses the existing lexical/module selectors, actual alias finalizer and mixed
Substitution. It retains complete target application metadata, or an explicit
unsupported demand for an unbound alias, without a second parser/evaluator or
inference algorithm. Source constraints and runtime use this same selection.
Runtime cached pattern targets retain the query's exact dependency guard; cache
entry checks relevant versions without source planning. Failed queries cannot
mint PreparedInputGuard; their Refused targets remain attempt-local and are not
retained in the runtime cache. Unsupported cold-name attempts may therefore
repeat metadata queries; a typed dependency-aware pending-result migration
remains held rather than retaining an unsound negative answer. Synthetic language-item
names retain their existing explicit source route; they cannot mint a source
pattern occurrence. No argument application is inferred from matching output.

The query does no IO, macro execution or body lowering. Name/type dependencies
are read cold under existing source/environment/epoch queries. Installed name
reuse checks retained stamps only. Direct syntax locations/Arc trees remain
shared; source indexes are deterministic. No new resource accounting/caps,
full-world dispatch scan, static checker, unsafe, query framework or backend.
Known legacy source/name/call routes remain named holds until replaced.

This stage does not deliver inferred record aliases. Their actual symbolic
alias target templates must later enter the existing source constraint graph,
including fixed/repeated/permuted/compound arguments, instead of losing those
relationships in independent holes. Exact-once runtime/cleanup, all pattern
contexts/standard macros, root solver/member dispatch and full fixture/task/
interpreter/JIT gates remain open. Source/control passes are separate evidence.

Acceptance: unchanged actual pattern nodes and target arguments across all
editions/original/user-emitted/wrapper source; real query/guard invalidation and
ordinary runtime evidence, no macro-name paths. Separate source review against
all eight recurring modes with MA002/MA003/MA006 constraints explicitly named;
no audit/task closure from this intermediate source/identity repair.

V403 exposed the missing prerequisite in canonical_module_declaration: its
Value-only projection was reused for record-pattern Type goals. Extend one
canonical_module_declaration_in_namespace metadata receipt with the actual
source-goal namespace. Preserve the Value-only wrapper for callable admission.
Use actual namespace candidates/declared-name receipts for original/generated
source; a Type goal grants no call authority. The existing bare opcode nominal
control lacks a published environment, so run its unchanged source through the
full coordinator instead of inventing source facts. Preserve V4039/14controls
and0/8targeted alias runtime rows as failed evidence.

V40414/15controls pass; the new exact-argument witness fails because build
classifies pattern names in its special metadata branch and skips classify,
where common path arguments are captured. Call classify for ordinary indexed
patterns as well as expressions after their binding metadata is captured.
This repairs the shared dispatch point, rather than adding another Type
parser/argument extractor or narrowing the witness.

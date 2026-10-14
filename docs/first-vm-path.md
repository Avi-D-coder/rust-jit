# T024 first VM path

The initial original-source component is now accepted. See
[the current acceptance audit](t024-acceptance-audit.md) and
[macOS evidence](../artifacts/macos-first/t024-source-phases-checkpoint.json). The following kernel/query
notes retain earlier checkpoints; their remaining-work lists are historical.


The first interpreter kernel consumes VerifiedRecipe only. Its frame owns a
structural body index, a symbolic PC, physical frame anchor, lazy value cells,
slot hints and an explicit active-scope stack. It has no parser or query access.
The kernel executes Literal, Unit, SetHint, ResultTransfer, ScopeEnter,
ScopeLeave and Unsupported. Next/Branch select a successor PC without fetching
it; Return emits an actual owning transfer ticket. Other dispatcher exits fail
closed until their assigned implementation tasks.

Every opcode increments its started counter before execution and completed
counter only after success. Exit counters advance only after successful exit
selection. Faults put the frame in a terminal state, prevent re-execution and
perform no further guest cleanup. Counters distinguish attempted operations
from completed operations rather than counting an entire recipe in advance.

ResultTransfer validates and allocates destination storage before moving its
source. A pure storage preflight checks the destination contract; rejection
preserves the source's ownership. Successful transfers use checked tickets and
cell state transitions. Scope entry follows structural ancestry. Scope exit
retires the initial storage inventory; general user Drop dispatch and normative
activation-order cleanup remain later cleanup work. Return cleanup belongs to
the dispatcher and is not claimed by the kernel alone.

Five actual kernel tests cover scalar cells and exact counters, reached
Unsupported terminal behavior, actual bool branch selection and retirement,
failed transfer atomicity, and explicit suffix/bool versus hint rejection.
These tests intentionally construct verified LRIR. The test source includes an
invalid cold function, but this alone does not prove source-level lazy queries.
No T024 source-program or cold-query fixture is claimed passed by these tests.

Still required for T024: complete source block/let/read phase lowering using the
pinned frontend; an explicit run dispatcher and return cleanup; execution of the
normative let/read program; actual cold-function query trace during that program;
structured CLI output derived from execution. BodyStructure/Thunk and initial
PC installation are now integrated as described below.
Generated-body and macro suspension/revalidation integrations remain in their
assigned tasks. T025 adds the complete numeric contexts/operations inventory.
Frame metadata and guest resource accounting need integration before broader
resource claims. Full release and Linux evidence remain pending.

The T024 query integration now adds BodyStructure and Thunk to the existing memo
database, with explicit stable key/value tags. Prepared original-source
publication derives a DefinitionSource input mapping each body definition to its
immutable ParseKey. BodyStructure observes the owning Signature, exact body
handle and source mapping; mismatched body identities/profiles fail. The private
pinned syntax adapter lowers one entered literal phase and computes a symbolic
parent return PC. Other currently unsupported entered phases produce legal
Unsupported recipes. No child or successor thunk is fetched by lowering.

Source movement preserves body-relative structural and recipe fingerprints;
an edited body rejects its old key. Query dependencies stay in the memo database;
these initial recipes do not claim embedded whole-worker revalidation proofs.
The installed-PC table owns verified recipe copies under its bounded metadata
budget and serves repeated PCs without entering the database. It is scoped to
one source publication; a new publication rejects table resolution. Recipe IDs
are local to the owning table and never public executable values. General
expansion-epoch revalidation remains M016/M038 integration. Execution must hold
the database's frozen run publication; this is exercised by the literal-to-VM
integration test.

Seven query/install checks exercise entered-header dependencies and zero cold
signature/body/thunk requests, symbolic successor non-fetching, actual literal
lowering, deferred Unsupported recipes, source refresh and stale rejection,
key/value separation, installed hits and quota rejection. The source-literal
integration test supplies scope entry explicitly and changes the source to prove
that the VM value follows the actual lowered recipe. It is not the normative
complete let/read program. The first rejected stale-key test run is retained;
its error-value expectation was corrected to the database's typed error outcome.

Remaining T024 work now centers on source block/let/read phase templates, run
and return-cleanup dispatch, full cold-function trace during main execution,
and structured actual CLI output. Numeric/source-profile, generated-body,
resource, release and Linux obligations above remain pending.

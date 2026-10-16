# Retained structural occurrence provenance — LOCAL-024

Pre-code bounded record, October16 2026. T024/T025/T032, M014/M018;
E-R5-005 source/scope identity prerequisite before common Type/call integration.
The unchanged primary witness is `auto-traits/auto-is-contextual.rs`, where
actual CLI f538 reportedly reaches the original `auto as u8` after expanding the
earlier item macro. This worker has not run that terminal. The other unchanged
runtime witness remains the complete suffix source and its full-payload routes.

Invariant: a current retained range must distinguish a structurally surviving
original occurrence from a newly emitted occurrence even when both use identical
text and copied token SpanRecords. Existing diagnostic projection is insufficient;
the existing `MacroDefinitionAnchor` already records this distinction. Existing
original declaration/macro anchors remain their sole authority until consumers
explicitly join a new proof. No origin guard is removed by this component.

Own only `expand/source_declarative/assembly.rs`, new sibling
`source_occurrences.rs` and its tests. Root owns source Type goals/prefix/finalizer,
DB/query/accounting/thunk consumers. Sole validator owns Cargo/native/test runs.

Extend the actual SourceSyntaxAssembly::new/attach_receipt/finish_scope path.
Initialize a bounded interval map over the actual initial normalized attachment
root. Each actual SyntaxEdit splits/removes/translates surviving fragments and
inserts the genuine output record/descriptor, parent inventory/invocation and
attachment ordinal with exact output-relative byte coordinates. Nested edits
retain parent fragments and identify the child replacement separately. Whole
ranges crossing differing/discontinuous origins are Mixed, never Original.
Caller suffix bytes carried by the adapter beyond the materialized output remain
Pending unless an actual structural mapping proves them. Token-origin equality
does not select any map entry. No parser, evaluator or mutable global registry.

PreparedSourceSyntax::occurrence(range) returns Original/Generated/Mixed/Pending
with checked bounds. Original coordinates refer to the INITIAL NORMALIZED syntax
root and its identity, not raw SourceBlob offsets. The root consumer must still
join actual source/root token context/edition/raw span and original AST within
the genuine owning member/current source lease. The retained map alone grants
neither Type/name lookup nor body/macro execution. Its identity and actual memory
capacity must be stable-encoded/accounted in PreparedSourceSyntax.

Cold cost is O(I) per actual splice over bounded intervals, O(I) retained memory,
and O(log I) lookup with one interval containment check; crossing origins return
Mixed immediately. Coalesce only exact adjacent contiguous provenance, including
same actual attachment ordinal, never same token-origin spelling. Check all
sizes/counts/arithmetic and reserve before construction. No edit replay, parse,
source planning, world scan or deep BodyIndex copy occurs on lookup/installed
execution. Root's pure Type query retains its completed occurrence result/guard.

Required source controls: original range moved after a real attachment; wholly
replaced/copy-origin output never Original; identical output bytes still Generated;
nested generated positions retain actual parent/child identity; changed containing
range Mixed; invalid/empty ranges and checked metadata budget. Use actual official
auto source and normal SourceSyntaxAssembly request/record paths; LRR controls
stay outside originals. All controls are initially uncompiled/unexecuted.

Eight-mode obligations: (1/2) syntax provenance only, no runtime evaluator or
macro-output executor; (3) no primitive/method/macro name special case; (4) actual
per-position provenance is the prerequisite for ordinary generated/source joins,
not a new origin refusal; (5) no patterns/cold executable demand; (6) retain direct
intervals, never replay edits during lookup; (7) immutable record/source/epoch
root, no namespace census/unchecked absence; (8) source tree/index remain shared,
no BodyIndex copies or whole-file node searches during use. Distinct MA findings
remain held. Exact source fingerprints and separate review follow implementation.
This is a staged source component, not architecture/feature/fixture/JIT acceptance.

## Implemented API and separate source review

`PreparedSourceSyntax::occurrence(SourceRange) -> Result<SourceOccurrence,
Diagnostic>` now delegates to one retained binary interval index.
`SourceOccurrence::Original { initial_syntax, range }` identifies the actual
initial normalized tree and coordinates. `Generated { record, descriptor,
parent_inventory, invocation, attachment, range }` identifies the genuine issued
output plus its unique actual attachment ordinal and relative bytes. Empty
ranges return Pending, foreign/reversed ranges refuse, and crossing differing
or discontinuous provenance returns Mixed without walking further intervals.
The public enum is descriptive data; it grants no Type/name/body/macro authority.

The map is initialized after the actual initial tree's materialization, before
any expansion. Only `attach_receipt`'s actual successful SyntaxEdit can splice it.
Copied SpanRecords are never inspected. Unchanged left/right fragments retain
their exact prior provenance and offsets; newly inserted output bytes receive
the actual record/descriptor/inventory/invocation and monotonically checked
attachment ordinal. Adjacent intervals coalesce only when origin, actual
attachment and relative coordinates are contiguous. Thus a removed node cannot
reappear as Original merely because emitted text/origins match. Existing outer
declaration and macro anchors are unchanged and remain authoritative consumers.

The exact materialized generated prefix is recorded. The adapter's statement
suffix can retain caller punctuation beyond that prefix; these bytes are Pending
until the adapter exports a real suffix mapping. A Type node lying wholly in the
prefix is unaffected. This component does not claim general suffix provenance
or a generated body boundary. The root must retain missing-proof states for
emitted/mixed Type occurrences until their actual expansion/scope join exists.

Both prepared source identity domains advance and explicitly encode the initial
root identity/length, current length, attachment count and every typed interval.
Current CapturedSource/epoch/seal authority remains `RetainedSyntaxInput`'s job;
no raw enum, context or digest mints those receipts. Ledger capacity is charged
at initial/new retained assembly state and prepared source metadata. Splice
preflights count/arithmetic and combined old/new allocation under32MiB, reserves
before writing, and validates contiguous full-root coverage/relative bounds before
replacing the old map. Source assembly consumes itself transactionally, preserving
its pre-existing no-partial-publication rule. No external mutation is introduced.

Source review against the exact leaves in
`worktracks/evidence/2026-10-16-retained-structural-occurrences-source.json`:

1. `SourceOccurrences::{splice,occurrence}` provide byte-position correspondence;
   no assertion/expression compiler or evaluator (MA001/MA007 remain held).
2. SourceSyntaxAssembly remains the ordinary retained source tree; no output
   executor or literal/break language subset (MA004/MA005 not closed).
3. No Type/member/macro name inspected by production provenance. SourceType
   authority/finalization remains separate (MA003/MA006 remain held).
4. Original/generated are determined by actual edits, never copied origin keys.
   Common generated/retained Type integration is still root work; no blanket
   unsupported origin gate is removed and no four-origin runtime claim made.
5. No pattern logic or callee/arm executable preparation; this syntax-only
   source assembly retains its existing explicit preparation authority (MA002).
6. Initial root identity is one O(N) cold tree hash. Each splice rebuilds O(I)
   bounded intervals and allocates O(I); lookup is O(log I) and reads one row.
   For A attachments with linear interval growth, cumulative construction can
   be O(A squared). This is explicit unmeasured cold cost, not a scaling/speed
   claim. There is no edit replay, parse, provider fetch or plan reconstruction
   in occurrence lookup. Root memo retains completed Type goals/guards.
7. No namespace access/census/absence grants. Existing actual issued-record,
   source/current-epoch/seal receipts remain intact. The metadata has no global
   registry or mutable authority cache.
8. No parser/BodyIndex/source-world copy or per-use AST search. Retain compact
   typed intervals inside existing Arc-owned PreparedSourceSyntax; charge actual
   Vec capacity with checked sizes, stable discriminants and explicit integers.

Five new source controls reuse the complete unchanged official auto input, with
LRR adaptations appended externally for forced motion, echoed item/copy origins
and repeated nested calls. They check original survival in all four editions,
forced movement, identical copied SpanRecord equality with Generated provenance,
actual nested attachment ordinals and mixed containing ranges, plus malformed
range/metadata overflow refusals. Tests run only normal request/record/attachment
paths; no original fixture is edited. They remain uncompiled/unexecuted.

Actual worker terminals: rustfmt completed; git diff --check completed. No Cargo,
native compiler, adapter execution, tests or guest runtime was run. Parent source
Type integration, all required runtime/milestone gates and obsolete-route removal
remain held. Preserve earlier callable/provenance-design review fingerprints and
all prior execution evidence separately. The minimal frozen payload is separate
from the moving shared checkout and has an exact recorded baseline/head.

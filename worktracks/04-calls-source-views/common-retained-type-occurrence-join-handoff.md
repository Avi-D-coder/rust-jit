# Cold retained Type occurrence authority join — LOCAL-024

Pre-code bounded T024/T032/M014/M017/M018 repair under E-R5-005.
Unchanged primary witness: `auto-traits/auto-is-contextual.rs`, actual root
CLI f538 reportedly reaches `auto as u8`. Root owns common entered Type goals,
shared finalizer and normal cast recipe; validator alone owns Cargo execution.
Worker owns source_body_context.rs and body_type_occurrences.rs, with only
origin-guard regions of source_local_plan.rs/qualified_type_names.rs available
if required. Do not touch the root's appended shared Type finalizer.

Invariant: source Type syntax in a retained macro-containing body uses the same
ordinary hygienic source/type pipeline when the actual Type position survives
expansion. Structural SourceOccurrence::Original must precede and be independent
of raw token span/source/root/edition and original AST/owning body joins. The
ledger's normalized initial coordinates are never raw SourceBlob coordinates.
An outer declaration anchor or copied diagnostic origin is insufficient alone.

Use the existing PreparedBodyContext::body_type_occurrence cold query service.
Return a borrowed immutable EnteredBodyTypeOccurrence view over its actual row;
origin/authority-demand methods reflect a fresh genuine authority join while
the underlying immutable syntax row remains NeedRetainedOccurrence. Existing
type_prefix and qualified Type consumers already use that service; no second
resolver, type parser, primitive/cast special case or new query framework.

Actual retained input/current epoch/source lease and owning original declaration
anchor remain required. For structural Original, project raw token origins only
after that structural fact, require genuine root context/unit/path/content/edition
for every covered token, then use the actual shared ParseFile of the original
source and exact PathType/record-path AST under the selected original body.
Compare ordinary nontrivia syntax token kinds/text without rebuilding strings.
No local/generic/import/module lookup or absence is granted by this source join.
Missing generated/mixed/provenance or original nested-body locator joins remain
explicit pending. Direct GeneratedSyntax keeps its existing real boundary proof.

Cold cost: immutable row binary lookup, relevant actual source/input reads, one
memoized original ParseFile demand and direct original AST range lookup, plus
O(path tokens) bounded comparisons. Existing actual source currentness is
unchanged and inherited. Query42/plan43 retain completed immutable results and
input guard; installed operations must not invoke this service. No BodyIndex
copy, whole-file per-use scan or global namespace census. Exact controls and
all-eight review/fingerprints follow; new behavior is initially unexecuted.

## Implemented source/API and review

`PreparedBodyContext::body_type_occurrence` still performs the existing actual
current source/lease check and immutable indexed-row selection. It now returns
`EnteredBodyTypeOccurrence`, a borrowed view dereferencing that SAME row. Its
origin/next_authority_demand methods consume `retained_original_origin`; no
caller-selected origin flag is accepted and no cached row changes. Both ordinary
unqualified and qualified Type paths already use these methods, so no duplicate
guard logic or edit to the shared source_local_plan finalizer was required.

For RetainedSyntax the join first checks actual source/body identity and actual
`PreparedSourceSyntax::occurrence(path_range) == Original`. Generated/Mixed/
Pending structural positions remain NeedRetainedOccurrence. It then checks the
actual original source/current lease, every projected raw token's unit/content/
path/root-context/pinned item normalization/owning edition, exact translated
outer declaration anchor and original body containment. Only those independently
projected RAW source spans locate the original ParseFile. Direct syntax_at
selects the exact original/current Path; PathType or record-constructor parent
kind, nearest actual function/const/static declaration and nontrivia token kind/
text sequence must agree. The normalized initial structural range is not used
as a raw source position anywhere.

Direct GeneratedSyntax retains its already admitted actual output/current
GeneratedBodyBoundary/hygiene/edition path. This slice does not expand generated
retained Type support, mixed paths or missing original nested-local body
locators; those remain pending with their real missing proof. Existing held
primitive_type_names cast scanner/global namespace paths are unchanged, awaiting
root normal query/thunk integration and actual removal evidence.

Separate all-eight review against the manifest's exact leaves:

1. New code certifies source positions/AST only, never expressions/assertions
   or runtime values; MA001/MA007 remain broader holds.
2. Both ordinary Type prefix consumers receive the same source view; no output
   evaluator, synthetic body or literal/break executor (MA004/MA005 held).
3. Production code reads no primitive/type/method/macro name whitelist. It
   certifies actual generic/path/record syntax; source Type finalization remains
   the parent's shared service. MA003/MA006 not claimed closed.
4. Structural survival and actual raw-source proof replace a blanket retained
   refusal for proved occurrences. Missing retained generated/mixed/nested-body
   joins remain typed pending, not bypassed or counted as accepted contexts.
5. No pattern logic (MA002), source Type resolution, bounds proof, child body
   index or executable branch/thunk preparation is added. ParseFile captures
   the actual already captured original source, not a selected live compile.
6. Cold join uses binary occurrence/anchor lookup, one original outline-item
   selection O(D), direct source AST locations/nearest-owner ancestry O(depth),
   bounded raw span projection and token comparison O(P). Original ParseFile is
   an existing pure memo. No immutable body rehash, whole-file AST walk or per-
   operation planning; query42/plan43 must retain the completed result/guard.
   Existing inherited RetainedSyntax currentness costs remain unchanged.
7. No namespace lookup/absence/world census is added. Actual RetainedSyntax
   currentness records source/seal/epoch/CapturedSource dependencies; the original
   ParseFile demand records real SourceBlob. No boolean or diagnostic span alone
   mints source/Type authority. Existing safety refusals remain intact.
8. Borrow immutable source/index/facts/parser handles. Direct syntax_at and
   binary interval/anchor indexes avoid whole-file searches. The view has two
   fixed fields, no new retained allocation/index or metadata registry. Existing
   source roots remain owned/accounted by the same plan; no BodyIndex clone.

Two source controls in retained_type_occurrence_join_tests.rs reuse the whole
unchanged official auto input across four editions and plain/retained sources.
They assert the actual u8 Type survives, the cached immutable row stays pending,
no Type/member/body/thunk work occurs, and wrong node/real epoch change refuse.
The existing source-control helper's owner registers the leaf. They are
uncompiled/unexecuted; no runtime or full-fixture claim is made.

Actual worker terminals: rustfmt succeeded; git diff --check succeeded. No
Cargo/native compiler/adapter/source tests/guest execution. Root query/finalizer/
normal cast recipe and obsolete production removal remain separate integration.
Frozen source has two production leaves and one test leaf, excluding the moving
root finalizer. Preserve all earlier failed sources and frozen evidence.

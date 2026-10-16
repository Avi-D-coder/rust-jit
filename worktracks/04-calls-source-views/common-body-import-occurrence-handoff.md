# Exact body macro import occurrences

Pre-code LOCAL-024 WT04 T014/T024/T032/M014/M017/M018 successor to copied context
prefix6 d38d5f6a/c4be5a/0. Invariant: the existing body import rewrite algorithm
must pass the actual selected import leaf to common textual macro lookup; a
(scope, offset, string) callback cannot prove a generated or grouped import token
context. Extend body_imports::lookup callback only, its two source_macros callers,
and cold PreparedBodyContext source occurrence rows. No second import/name resolver,
root-context guess, source-origin refusal or per-operation source preparation.

Actual parsed UseTree exact range plus already captured BodyImport/SourceImport
identity/leaf/range/path select the one unqualified target token for the existing
textual case. Alias names never supply target context; `use foo::{self}` uses its
actual ancestor path token. Unsupported/ambiguous occurrence authority stays typed.
Rows retain actual source admission identity, raw token context/edition and exact
leaf stamp, sharing parser/index/admission roots. Cold O(import paths/depth) once,
checked vector/string accounting in PreparedBodyContext exclusive metadata; hot
selection chooses retained rows and existing MacroDeclarations using real body
expansion ancestry. Original graph callback remains the same adapter. Retained
mixed-expansion reference origin remains a separate structural occurrence gap.

Unchanged stock Point/const/enum witnesses and imported originals remain unchanged;
LRR generated import/alias/wrapper/currentness controls live outside originals.
Root owns BodyContext query/pins, CLI/run and production constructor switch. No
Cargo/native/extra agents/commit/ledgers. All8 exact source review/manifest follows
implementation; no runtime or feature acceptance is implied.

## Exact source handoff

Source6 fingerprint
`6c500ea260a522ed3f66d9c84494401e533183aaac6d04d08bd1287c1ecd81ac`
with all paths/hashes/bytes is in
`worktracks/evidence/2026-10-15-body-import-occurrence-source.json`.
Status: uncompiled/unexecuted source-only. This succeeds copied context-prefix6;
constructor source3 remains separately held. Root CLI/run/query/memo sources are
not included or edited by worker. The helper is a nested module of
source_body_context, so no frontend/mod.rs edit is required.

`body_imports::lookup` passes the actual `&BodyImport` to its textual callback.
The normal and linked source_macros callers pass the same row into
`EnteredMacroIndex::resolve_import`. For a prepared body,
`PreparedBodyContext::macro_import_occurrence` binary-selects an opaque retained
row from the cold `PreparedMacroImportOccurrences` plan; actual source admission
identity, source import identity/leaf/scope/at/range and target spelling must match.
Its raw context/edition reaches `resolve_namespace_with` and the existing
`MacroDeclarations::resolve_with_outers`. The prior prepared-import refusal is
removed because the actual occurrence now exists, not by substituting root
context. The original metadata adapter supplies the same old original root context
and uses this same resolver/import rewrite; no duplicated import algorithm exists.

Cold preparation uses exact ParsedFile::syntax_at(UseTree) on the already admitted
SourceImport leaf. It visits only its actual path/UseTree ancestor chain, bounded
to 256, to retain the actual target NameRef. Renamed alias tokens are excluded.
`use foo::{self as alias}` selects its actual ancestor foo target. Other qualified
paths remain with the existing common import algorithm; this helper does not
parse a second import language or choose namespace semantics. Actual original
context None is admitted only by the existing original source/parse receipt;
generated/retained target tokens must have their actual attachment. Syntax-context
normalization stays in pinned hygiene/MacroDeclarations. No diagnostic source
origin is interpreted as expansion order. `reference_origin` still requires the
actual generated output record and retains the genuine mixed-retained/captured
parent occurrence gaps.

Metadata: the immutable plan stores only small numeric source stamps, ranges,
raw context keys, editions and target strings. No ra_ap_syntax handles, parser tree
or BodyIndex copy is retained in it. Vector/string capacity is checked/accounted
in PreparedBodyContext exclusive bytes; shared source/parser/index/input/boundary
roots and their existing accounting APIs are unchanged. The new rows add no
separate memo or cache key. Cold cost is O(import count log(import count) + total
selected path depth), once under root BodyContext query. Hot proof selection is
O(log(import count)) plus bounded actual source identity checks. Existing
body_imports module/prefix construction remains cold source preparation debt;
it must not be invoked during installed runtime dispatch. An owned cached local
plan or exact occurrence query remains necessary for constructor production.

External controls are strict and source-aware. The frozen112v2 opaque-offset
failure2cce3b/1 and later typed assertion failuref14aec/1 are preserved. The latter
is expected source refusal: unresolved item producer `pending!()` is installed by
SourceLocalIndex::local_index_inner into block-wide item_frontiers. Type items
are unordered and its possible output can affect Local/Alias even before its
textual invocation. lookup_ordinary checks that frontier before Type candidates;
no Type fallback is justified. The new control exercises two actual source states:
with an unresolved producer it requires the EXACT NameLookup/LocalLookup NeedInput
identity; without that producer Local/Alias must be positively Defined. It queries
an actual ordinary value occurrence, never the next opaque item boundary. It keeps
real local binding, foreign PC and header/currentness refusals.

The generated boundary control adds actual alias import callback lookup across
arbitrary emitter/wrapper contexts, and a valid grouped module self-import for
pure target occurrence proof. It refuses wrong target, wrong leaf ordinal and an
unattached source admission identity. Source declaration names in these controls
are not language dispatch keys. The control inspects real issued syntax/source
receipts and lookup; no guest/stock main or supplemental identity runtime pass is
claimed. Unchanged primary Point/const/enum originals are untouched. Official
rustc `tests/ui/macros/nested-use-as.rs` was also source-reviewed for its real
2018 imported macro-alias shape; it is a check-pass source reference, not a newly
imported or executed runtime fixture in this packet.

Eight-mode review at exact source6 (distinct from MA001–MA008):

1. This is source occurrence preparation/lookup only; no assertion operand
   compiler, wrapper or evaluator. MA001 is not closed by this component.
2. Actual UseTree/NameRef/BodyImport source identities reach ordinary lookup;
   no generated literal/break payload executor. MA004/MA005/MA007 remain separate
   held engine obligations.
3. The existing body_imports rewrite and MacroDeclarations hygiene resolver are
   retained, without macro identity, method or numeric rules. MA003/MA006 are
   unaffected; no fixture spelling controls semantics.
4. Prepared generated import lookup now receives its actual token proof through
   the common callback. Remaining mixed-retained expansion-order and captured
   parent occurrence transport gaps stay typed; no invented context grants access.
5. No pattern evaluator or cold executable checks are added. Full lexical facts
   retain existing canonical binding/source scopes and unresolved producer demands;
   MA002/MA008 closure is not inferred.
6. Exact import occurrence plan is prepared once and retained. Callback proof
   selection does no parsing or plan construction. Root pure-query/entered-plan
   ownership and existing import-prefix memoization remain named prerequisites
   before installing this preparation into execution; per-op rebuilding is not
   installed by this component.
7. No new world namespace construction/comparison is added. Existing module/import
   algorithm remains cold and uses its existing graph service; replacing its
   relevant input transport is root-owned. No unrelated-invalidation or measured
   performance claim follows.
8. Exact source locations/ancestor paths replace any need to search the whole file
   for an import occurrence. Parser/index/authority roots remain shared; the plan
   retains numeric data, not cloned immutable body/catalogue structures.

Actual syntax formatting6bc030/0,1987b1/0,fb7013/0 and whitespacef20ad2/0 only.
No Rust/Cargo/native tests were run by worker. Sole-validator required reproduction:
current/pending exact owning-context test in four editions; generated boundary
actual alias/wrapper/currentness and wrong occurrence controls; existing body import
and namespace banks; then root common source entry and eventual all-context engine
traces/full upstream runtime contracts. Unsupported/unexecuted std/Box/library and
constructor body-Type/local-plan integrations remain gaps. No architecture, task,
feature or milestone is marked complete.

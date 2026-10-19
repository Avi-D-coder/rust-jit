# Separate source review — local derive selection and admitted insertion

Solo root, LOCAL-049; parentfed34ceb5605a5fea8ef9d10cea347148ddb6f30. Reviewed
V257 fingerprint `95e2cc42311e21c138ee2ad1a430ede8bd0e95f68d228cc7c662db7a1167089a`
and V258 `9e9b8e3d60ff97127852684c60bfe0d824358358d83d048c7f5d6737c620c6c7`.
This is a held M019/M020 prerequisite, not integrated local-derive completion.
Source/compiler/runtime/full-fixture outcomes remain separate.

## Actual route and provenance

PreparedSourceView::original_annotation_producer retains a pure ordered map of
existing module producer IDs and actual DeclarationScan attribute IDs to source
ranges in the existing publication_prepared cache. Its key binds location,
parser/profile and cfg. The cache owns only digests/ranges, never a SourceView or
its own publication, so no ownership cycle. Selection then uses the same real
Outline or cached lexical declaration plan/row as ordinary entry. Missing local
parents remain pending, never an empty impl/candidate conclusion. There is no
macro-name-based production selection or new producer hashing rule.

source_derives::prepare_group no longer finds only a module producer before
using the shared target. It uses actual paths, compiler/procedural definitions,
helper declarations, source-occurrence inputs and existing constructors.
GeneratedDeriveExports::from_source checks the supplied ItemOutline against that
actual producer target. SourceAttributeSelection::from_resolved also uses the
shared target, retaining its actual cold scan/grammar when not a module item.
No local compiler-Test registration is invented. Execution of procedural output
and replacing attributes after a local derive remains unexercised.

DeriveDeclarationOrigin::source_target validates actual source selection,
producer-to-item attachment, canonical definition and header. validate_database
still reads CapturedSource, rejects GeneratedSyntax as original provenance and
fetches the ordinary Outline query (retaining its parse/cfg dependencies), then
requires the real retained prepared-source view and shared target. The old
module-only attribute_producer membership check is removed. A generic ledger
seal or arbitrary row vector cannot provide this source view. SourceDeriveSelection
also checks its retained publication and actual canonical scope before supplying
an original insertion target.

GeneratedDeclarations::admit_source_derive_output obtains that private real
target before the existing admit_attached kernel. Original items can have a
Statements annotation context but emit Items. The parent scan row and its
lexical scope kind remain unchanged; the authenticated item supplies canonical
insertion scope. The source/target and parent parser/cfg/path, original source,
attribute attachment and item range are checked. This call constructs both
parent scan and target from the same retained source (including unit/module
identity); callers cannot construct OriginalItemTarget fields. Other admission
entries retain their exact previous grammar matching. All record/key/epoch,
call edition and token/context checks remain. No fake generated source or
modified scan row is used to bypass admission.

V257 exposed GeneratedHeaderPlan's remaining blanket owned-scope refusal after
selection, origin validation and real output admission succeeded. V258 extends
its SAME prepare_using with an admitted GeneratedDeclarations receipt.
prepare_admitted derives scope from that receipt and checks exact record,
epoch, cfg and scope before reading headers. Bare owned scope keys still fail;
hygiene marks cannot supply insertion scope. ParsedFile::outline_item_fragment
validates admitted item grammar/canonical module path and calls the same
outline_in_root kernel. contained_scope now recognizes the actual local root
owner while continuing to exclude unrelated associated owners. Ordinary module
source derives use prepare_admitted in append_source_derive_output as well.
Actual generated impl range/header projection shares the same retained
ActiveImplProjection path; no replacement range-only acceptance route is added.

## Eight recurring failure modes

1. **Assertion evaluator (MA001/MA008):** unchanged; no assertion/compiler/VM
   operand special case or synthetic executable source.
2. **Restricted generated execution (MA004/MA005/MA007/MA008):** selected expansion
   uses the pinned builtin adapter and real token record, GeneratedItems,
   GeneratedDeclarations and ordinary header projection. No output whitelist,
   direct scalar/break executor or runtime macro semantics.
3. **Context-specific methods/numbers (MA003/MA006):** no method/type/inference
   algorithm or fixture spelling in production. Existing exact builtin derive
   definition selection is reused, not a Copy/Clone runtime shortcut.
4. **Origin exclusions:** module-only original producer lookup/replay is removed
   in group selection/helper origin. Authenticated local item insertion replaces
   a proven header-preparation owner refusal; unproved scope keys keep their
   refusal. Generated originals still require real expansion provenance.
   Generated functions/types/macros, local nominal publication and all origin
   matrices are not claimed complete.
5. **Patterns/eager bodies (MA002):** canonical declaration plans supply metadata;
   no body entry, bindings, pattern engine or executable lowering is added.
   Cold producer expansion is required by Section50.3. The new control traces
   actual preparation and checks no BodyStructure/Thunk/PatternPlan/signature
   query. It never turns catalog failure into a runtime/source pass.
6. **Repeated planning:** attachment maps are retained under actual immutable
   publication keys; parser/canonical plans are shared. No new parser/evaluator
   or installed-operation source work. Existing body identity validation,
   repeated derive-group/token-forest replay and header preparation caching
   debt remain; complete prepare-once/performance acceptance is not established.
7. **Global scans/invalidation:** one cold attachment index reads module rows and
   source scan producers. Lookups use ordered index keys, with actual source
   target/header validation. No namespace census/guard added to dispatch,
   installed recipes or ordinary call entry. Relevant semantic solver/cache
   invalidation remains unverified; no performance claim.
8. **Immutable copying/searches:** shared Arc plans and original parser/direct
   locations remain. No BodyIndex deep clone or per-phase full-file syntax
   search is added. Generated header projection uses the existing kernel.

## Limits and next integration

The configured-source control uses actual captured facade/compiler namespaces.
Its cfg(test) inspection method replays ordinary prepare_group against those
names and actual helper declarations, using no manufactured exports or completed
occurrence receipts. It expands only the genuinely selected first builtin,
validates origin and actual admitted output/header insertion, rejects changed
header/producer and bare coarse/local scope keys, and reports the actual catalog
result separately. It does not complete a producer, append results to a source
worklist or execute a guest. The same unchanged fixture remains in the failing
whole catalog matrix; no exclusion or skip is introduced.

Required next work is complete item-group ownership and common cold macro
prefix/adoption. Multiple Meta jobs must map to the genuine ordinary item's
producer; running a whole derive group for each annotation would duplicate
execution. Inspect and reuse DeclarationMacroIndex and
GeneratedDeclarationMacroIndex's existing compiler_outers/derive_prefix_view/
adopt_completed_child_prefix machinery before extending another body environment.
GeneratedMacroCatalog::for_discovery/contained_scope, SourceMacroIndex,
SourceDeclarative prefix/completion and SourceDiscovery calls,
frontiers/restoration still assume module ownership. No completion is possible
by dropping output macro bindings or claiming inert metadata. Local associated/
container targets, replacing attributes/procedural execution, full source-goal
translation and actual upstream inference/delegate/root solving remain open.
Global catalog/M019/M020/interpreter correctness/runtime and JIT gates remain
held. Actual terminal outcomes accompany this source review.


Final terminals: V257 builds pass and 8/10 controls pass; preserve the local
control's real header-owner failure. V258 builds pass and 9/10 controls pass.
The new control passes all four editions, actually admitting the selected
upstream builtin output and projecting its one impl header, with all source/
header/producer/coarse-scope controls and no executable body queries. Existing
copy direct/user-macro/wrapper runtime regression passes 12 worlds. The complete
global source matrix remains FAILED: 8/12 source worlds succeed and the local
derive producer blocks all four editions. No local derive producer is settled by
the component control, and no new distinct stock runtime gain or refreshed
suite total is claimed. Source/HEAD/binary guards match throughout both runs.
The final outline.rs diff includes rustfmt-only reflow outside the new
outline_item_fragment method; that formatting is included in the validated
fingerprint. Plan/spec/source checkpoint checks are recorded separately.

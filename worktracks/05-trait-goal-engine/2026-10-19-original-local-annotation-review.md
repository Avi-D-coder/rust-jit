# Source review: retained original item targets and lexical plans

Solo root, baseline04bf4384c7471670a5ea6d64b65c578f374d2bdd. Reviewed source
fingerprint `34ae71db663f0db2ef02e4c62f2161bf80630b698d276c034b2175829d75a185`
(V255). Component candidate; global catalog/local-derive integration remains
HELD. Test terminals and limits are recorded separately.

## Shared paths and removed work

`ParsedFile::declaration_scope_plan` and `entered_container_facts` still validate
the real retained body/container before the single `lexical_declarations`
collector. That collector now retains one `Arc<DeclarationScopePlan>` per
parser/body/cfg/scope/header-context key in the owning parser. The exact previous
stable plan key domain/fields are preserved. The mutex serializes only cold
construction; no guest call or dispatch takes it. `LocalImplOwners::prepare`
retains that same Arc instead of constructing a new wrapper. Ordinary
`entered_macro_facts` calls `prepare_entered` on it and prepares bindings only
then. `DeclarationScopePlan::items_by_range` is a derived ordered exact-range
index of the actual collected items/members, built once. It grants neither body
entry nor completed annotation/impl authority.

Deferred bindings retain pinned `SyntaxNodePtr` locations instead of
thread-affine red AST nodes. The plan owns a cheap clone of the pinned Parse
(green-tree handle), not a clone of ParsedFile or BodyIndex. Resolving pointers
uses the same retained syntax root; a private request cannot select another
parser. There is no new unsafe, leaked cache, TLS, self-owning Arc cycle or
caller-supplied declaration list. Parse -> plan -> green tree does not point
back to the owning ParsedFile/cache. A new source publication/parser or changed
cfg/body/scope selects a different plan. Existing body validation remains before
a cache hit. In particular entered_body_root recomputes fn_body_identity over
the selected body; container entry similarly checks body_content_identity. That
existing O(body syntax) validation is NOT removed by this candidate and must not
be described as constant-time entry or complete prepare-once performance
acceptance. The new cache avoids rebuilding declaration/header/deferred-binding
plans, not every existing source validation walk.

`PreparedSourceView::original_item_at` keeps actual module Outline references
or a local plan and real item range. Local target lookup follows genuine
BodyHandles and canonical scopes, using the same collector as entry. It never
uses the declaration scan's coarse function owner as a replacement item owner.
`original_annotation_target` attaches the actual Meta via the retained parser.
All current module derive and procedural attribute item selectors now read
`OriginalItemTarget::item`. All four former whole-source `ParsedFile::parse`
calls in source_derives.rs and source_attribute_requests.rs are gone; selections
read `retained_declaration_parser` instead.

Cold costs: one lexical walk/index per actual plan key, retained with the parser;
plan lookup hashes the supplied header context/cfg and uses an ordered map.
Module target selection scans the Outline; nested target selection reads
prepared declaration rows to find the next genuine enclosing body, then resolves
the selected row through its range index. These are cold annotation/source
preparation costs, not per-operation guards. Direct pinned syntax pointers resolve
by source range in the retained tree, without a complete-file descendant search.
No measured release speed/scaling claim follows from this inspection.

## Eight recurring failure modes

1. **Separate assertion evaluator (MA001/MA008):** no assertion operand/runtime
   route changed or introduced. All changes concern actual source metadata.
2. **Restricted generated executor (MA004/MA005/MA007/MA008):** no scalar/literal/
   break executor or output whitelist. Generated output remains with ordinary
   admitted syntax and existing publication; this candidate does not complete
   body-local derive output publication.
3. **Context-specific selection/inference (MA003/MA006):** no method-name, macro-
   name, Copy/Clone or numeric rule. Source tests select actual annotation ranges;
   production target construction does not branch on a fixture or derive name.
4. **Source-origin exclusions:** the new target explicitly identifies ORIGINAL
   source; its current callers are existing original derive/attribute services.
   Generated services remain distinct in admission provenance, not execution
   semantics. No generated runtime syntax refusal is added. General generated/
   wrapper/standard-macro local target integration is not established here.
5. **Patterns/eager cold bodies (MA002):** declaration collection captures only
   deferred binding requests. Cold annotation lookup issues no BodyStructure,
   Thunk, PatternPlan or FunctionSignature query and does not apply bindings.
   Ordinary entry applies the existing BindingBudget/collector. No new pattern
   machinery, cold-arm executable validation or static compiler check.
6. **Repeated planning:** cached immutable plans share facts across cold/entered
   readers. The four alternate parser calls are removed. Existing module-only
   producer/group lookup, repeated token-forest serialization and downstream
   coordinator replay debt are not claimed removed.
7. **Global scans/invalidation:** no namespace census, global-map comparison or
   new installed-recipe guard. The new Outline/local-row reads occur only in cold
   target preparation. Exact source selection and actual parser identity remain
   required. Full semantic positive/negative/currentness acceptance is open.
8. **Immutable copies/whole-file searches:** no BodyIndex/IR deep copy. Plans and
   fact payloads share Arc handles; deferred requests use pinned direct syntax
   locations. Original target retains its real immutable source lifetime.

## Explicit remaining integration

`OriginalAnnotationPlan` and compiler preparation still enumerate module
ordinals. `source_derives::prepare_group` still locates its initial producer in
module Outline before using the common target. `DeriveDeclarationOrigin::Source`
and completion/original-macro proof replay still assume module provenance.
`SourceDiscovery::append_source_derive_output` requires its ordinary `.calls`
entry; a nested scan job needs a real common item handoff, not an invented call.
`complete_source_derive`, source graph restoration and prefix publication also
need that actual owner. Local module/trait-container nesting which has no known
canonical body parent returns no target; this is an explicit metadata gap,
never a fake negative trait result. No existing cold attribute path is made
conditional on this unfinished metadata join.

The unchanged local enum target and module Foo target are checked across all
four editions, including real declaration/facts Arc identity against ordinary
entry and changed-source selection. The existing unchanged local Drop witness
checks deferred bindings after pointer conversion. Runtime origin controls and
the prior complete catalog matrix are selected separately; results must come
from collected terminals. Matching final values alone cannot establish the full
architecture matrix. Full M019/M020, source-to-upstream solving, runtime and JIT
gates remain incomplete.


V255 actual terminals: library and CLI builds pass; 9/10 exact controls pass.
The original failing catalog matrix still fails in all four local-derive worlds,
with the other eight source worlds successful. The unchanged copy contract runs
successfully direct/user-macro/wrapper in all four editions (12 runtime worlds);
this is an existing regression check, not a new distinct stock gain. Source,
HEAD and binary guards all match. No skipped or partial case is counted passed.

Separate V256 source review, fingerprint
`be2b6e283cf13a074b5165c20375df8651534d0967e8a7c749f8e3da99d0e351`:
the only production delta from reviewed V255 is exact Meta-to-header attachment
validation in original_annotation_target. An arbitrary range inside the same
item cannot masquerade as its annotation. The unchanged local fixture adds this
negative range check. This is cold source identity validation, with no new
parser, item owner, authority or runtime semantics. Recheck both target controls,
local Drop deferred binding identity and changed cfg/source scheduling. The V255
runtime/origin/global-matrix terminals remain evidence for V255, not an assertion
that V256 reexecuted them; their relevant runtime/solver code paths did not change.
The broader milestone and cold derive integration are still held.

# Shared alias/nominal declaration Type authority

Pre-code bounded successor LOCAL-024/WT04 T014/T024/T032/M014/M017/M018,
October15 2026. Actual immutable V2 namespace21 terminal706798/1 was20pass1fail:
generated_named_reference_uses_the_same_positive_source_type_receipt_all_editions
fails captureNone after current signature Type lookup succeeds. AliasNormalizer
re-resolves the selected nominal declaration's own name using an older original
producer graph before its ItemKind nominal check; it returns NeedProducer.
The violated invariant is current owning declaration Type authority, shared by
ordinary, retained/generated, alias and nominal uses. Do not move the nominal check
ahead of name authority, infer source from layout, or fake a FunctionSignature for
nonfunction declarations. Frozen V2 evidence remains unchanged; pending field-origin
source is a separate unfrozen successor and outside this bounded packet.

Extend AliasNormalizer::normalize_traced self-name and single-name alias target
lookup through existing SourceTypeOwner/current Type authority and root's shared
declaration local/with_lookup classifier. Factor generated_result_namespace's
current source/header/origin/Type receipt core to accept actual declaration facts;
function/reference wrappers preserve their parser/signature/body checks. A generated
nominal/alias consumer must compare the actual existing pure TypeSchema/TypeAlias
memo recipe before entering that same core. All original/retained/generation scope,
header, edition, source origins/independent definer scope, current seal/epoch and
selected canonical positive identity checks remain. No parser/evaluator or new query
framework. Old stale graph gates are superseded only in these shared consumers.

Cold work: existing memo header/schema/alias facts and one current Type bucket;
selected names/aliases only, no body execution/preparation. Keep bounded alias-cycle/
depth/trace bytes and source-world currentness. Namespace provenance added by current
receipt lookup must be represented in alias dependency trace (or its conservative
existing source-world certificate); do not leave an untracked lookup. General
opaque/mixed-site Type origins remain explicit holds where current proof is absent.
No runtime hot operation gains queries, full-world map scans or body copies.

Primary unchanged witnesses remain regions-infer-borrow-scope.rs Point x_coord,
regions-infer-call.rs and existing official alias fixtures. New LRR controls outside
originals must exercise actual generated publication with stale original graph,
positive nominal completion and alias target lookup, shadow/foreign/currentness
refusals, cycle/depth/dependency trace and no BodyStructure/Thunk demand. Separate
all8 source review, exact hashes and sole-validator execution required before any
acceptance; no Cargo/native/extraagents/commits/ledgerclosure by this worker.


## Implemented source-only alias5 and separate source review

`AliasNormalizer::normalize_traced` now performs its self-name lookup through
`type_name_namespace::lookup_declaration_name` before the existing nominal ItemKind
completion check. Exact canonical positive identity is still required. The stale
stored graph route is removed, including the now-unused graph field; constructor
`AliasNormalizer::new` still validates the actual source catalogue against its
supplied graph. Single-name alias targets use `resolve_declaration_name` and the
root's declaration local/with_lookup classifier. Generic/owner/compound/symbolic
and cycle/depth outcomes retain their existing contracts; no nominal early-return
shortcut or primitive spelling fallback is introduced.

`type_name_namespace::{lookup_declaration_name,resolve_declaration_name}` generalize
actual SourceTypeOwner authority, using the existing current original/retained
proof or actual generated Type declaration proof. Local/generic/owner priority is
the same shared classifier as function signatures. Original fallback preserves
nonready lookup outcomes while refusing to turn Absent/Defined into completed
namespace authority when imports remain incomplete. Expected-name wrappers still
refuse every noncompleted or mismatched result. This preserves typed producer/
ambiguity outcomes without allowing incomplete absence to become a primitive.

`generated_result_namespace::lookup_declaration_inner` is the shared current
source/header/origin/Type receipt core now used by existing function/reference
consumers and `lookup_type_declaration`. The new nominal/alias entry selects the
existing pure TypeSchema/TypeAlias query from the actual canonical GeneratedSyntax
DefKey kind and compares exact definition/scope/header against that memo recipe
before the core. ItemKind chooses a source parser proof, never semantic completion
before lookup. No synthetic function signature is fabricated. Current source,
declaration range/header/profile/scope, exact expansion record, epoch/seal/current
Type scope, positive/negative bucket and independent original/parent definer-origin
checks all remain. Function wrappers retain body/reference occurrence and historical
signature replay checks. Generated alias declaration capture retains the actual
published nested declaration scope with canonical unit/module-prefix checks, instead
of fabricating the expansion unit's top-level scope.

`syntax/type_aliases.rs::ParsedFile::type_alias_recipe` now uses exact indexed
syntax_at(context.range) selecting only TypeAlias, then the identical exact range
and header token match; its former whole-file descendant scan is removed, without
fallback or changed error identity. The existing pure TypeAlias memo owns this
source recipe; schema/alias preparation does not enter executable bodies.

The only runtime consumer of normalize_with_dependencies is
NormalizedDeferredReturn::mint. Its existing captured Database revision and source
revision guard invalidates any changed Type receipt, record, epoch or namespace
publication, in addition to explicit source/alias query trace rows. That conservative
guard is retained and documented; the trace alone is not represented as a complete
new namespace certificate. Relevant bucket-only invalidation and removal of these
existing broad revision/census guards remain independent architecture debt. No
untracked installable alias proof is minted by this change.

New source-only controls cover original and generated alias-to-nominal chains under
actual completed discovery with a stale historical original graph, actual generated
alias-to-primitive scalar result/GenericEnv capture plus epoch rejection, and wrong
header/foreign scope against the actual memo-owned generated Type declaration.
They run across all four editions where specified and assert no BodyStructure or
Thunk demand. Existing V2 failed generated named-reference test is unchanged, as
are the prior passed primitive/static and namespace authority controls. These are
LRR supplements outside licensed originals; they are not whole-stock passes.

Separate all-eight review of this five-leaf fingerprint:

1. Only ordinary source declaration Type services changed. AliasNormalizer,
   type_name_namespace, existing generated source authority and ordinary signature
   classifier compose; no new assertion/expression evaluator. MA001/MA007/MA008
   alternate production macro routes remain held elsewhere, not removed here.
2. Source admission is independent of literal/break body shapes and macro identity.
   Nominal/alias header proofs are pure memo facts. MA004/MA005 existing output
   shortcut removal remains separately held; no output execution was added.
3. Builtins require actual completed Absent through the same declaration classifier;
   positive nominal identity must equal the selected canonical definition. No method
   or numeric name whitelist or layout-based source inference. MA003/MA006 held.
4. Actual original, retained or generated source owns each Type lookup. Generated
   nonfunction declarations now join the existing source/header/receipt core using
   their own real recipe; no original-only rejection was added. Unsupported opaque/
   mixed-site/foreign scope authority remains explicit rather than guessed.
5. Only the reached alias chain's header recipes are demanded. No executable body,
   unselected branch or runtime pattern is prepared. Cycle/depth limits remain.
   MA002/pattern work is disjoint; all relevant runtime matrices remain unexecuted.
6. Existing TypeAlias/TypeSchema pure memo queries retain preparation and dependencies;
   generated nonfunction entry compares memo recipe instead of reparsing a function.
   Alias recursion still resolves one reached target at a time, with bounded depth.
   Existing cold original namespace fallback and historical generated result replay
   costs remain held debt, not a prepare-once or measured speed claim.
7. Current Type scope lookup borrows one selected canonical bucket. No dispatch,
   runtime operation or cleanup gains a namespace scan. Existing cold source
   catalogue/fallback/census preparation and conservative whole revision invalidation
   remain named exceptions; this is not mode7 acceptance for all source call entry.
8. Exact selected TypeAlias syntax lookup replaces the old whole-file search.
   No immutable BodyIndex/parser clone or new index/cache framework is introduced.
   Nominal/alias recipe comparisons borrow existing memo-owned allocations; existing
   metadata/alias trace bounds remain. Parent/peer source-storage/index changes
   require their own coherent combined-source review and validation.

Formatter --check terminal3f413c/0 and targeted diff --check2f7d7c/0 only.
All new alias5 source is **uncompiled/unexecuted**. Sole validator should rerun the
actual V2 namespace21 bank, new nominal_and_alias/generated_alias/generated_type
controls, ordinary alias cycle/depth/producer/dependency tests and normalized return
currentness/wrong-world controls. Full generated member fields, field-origin packet,
opaque adjustment, stock x_coord full-main Box/standard library macro/runtime,
suspension/panic/cleanup matrices, all8 architecture and milestone acceptance remain
explicitly open. Pending field source files syntax/fields.rs and
engine/run_field_source.rs are UNFROZEN/UNROUTED and not part of this alias5 packet.

Alias5 final fingerprint `12efb1542a1201252dee8808a90b2896fa9efa9d680212826c8f34f4448786b8`; encoding is SHA256 of sorted
`sha256 + two spaces + relative path + LF` rows:

```text
5c1212c19eb78179d86968f62faf4186306160258e3a3f55a7c6067c2550300a  crates/lrr/src/frontend/generated_reference_source_tests.rs
1f3899e6a14b2b1e5ab90d01782e1ab378c0128fd2e326aeb4897882be071ea2  crates/lrr/src/frontend/generated_result_namespace.rs
2921e2598ca33e87049d13561c88dd45ac0f8b66a8610ebe52bfd72854297c68  crates/lrr/src/frontend/type_aliases.rs
26bd9261dc42e3b7e2db3bc07016649102ccf77965de417491872256e97678ee  crates/lrr/src/frontend/type_name_namespace.rs
981963ada18150f4419febe2ba5a13656493da62b372d3f9e61f31e6d1426427  crates/lrr/src/syntax/type_aliases.rs
```

Root-owned shared declaration classifier dependency signature_names.rs SHA256
`f4d8de877c270ef20b2ea5b36bde5b9954b6bc17034a99c67e23624670060976`. These five worker leaves are frozen pending parent copy;
no edits until copy confirmation. Old immutable packet evidence is preserved.

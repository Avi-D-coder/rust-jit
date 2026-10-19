# Demanded trait candidates — source review, gates held

Solo /root; baseline fe104a8ad466384ace6468f1a23fa7c1d94ce435.
V316 reviewed source fingerprint:
`c8dda2072f8f2d03c6f4ef4cce444b55efbc0f097540dbdfc677b9f4615879b6`.
Review conclusions are separate from collected test terminals.

1. No assertion/expression evaluator. `source_types::projection_trait_identity`
   is the extracted identity prefix of the existing ordinary projection/bound
   walk. Both old projection arguments and new impl membership use that same
   parser/name/trait-declaration service. `ImplDeclarationContext::observe` is
   likewise extracted from existing ImplHeadTypes canonical metadata observation.
   There is no new interpreter, macro-specific entry or runtime dispatch.
2. No literal/break-only output execution. `PreparedImplTraitIdentity` consumes
   genuine ImplHeaderSyntax and its actual canonical owner, retained syntax and
   declaration rows. It transports names/metadata only, not guest execution.
   Generated/local headers use the same query. MA004's complete output execution
   requirement remains separate, not closed by this source component.
3. No context-only methods or numeric inference. Trait declaration identity is
   obtained by the ordinary unqualified/qualified Type service, with actual owning
   editions and generated token contexts. Generic arguments, self Types, where
   predicates, applicability and adjustments are not guessed from names/runtime
   values. Negative/conditional/blanket impls remain actual raw candidates; the
   pinned solver must decide applicability. No custom solver/canonicalizer/SCC.
4. No new generated-source exclusion. Canonical original/generated source is
   consumed through existing inventory/metadata proof. The32-head equivalence
   matrix compares actual generated and original identities in all editions.
   Arbitrary mixed-edition/foreign/generated/import/associated/parent contexts
   remain unexecuted or incomplete; this does not claim the full origin matrix.
5. No duplicate patterns or eager executable branches. The identity-only request
   never calls ImplHeadTypes/ImplPredicates or the old Type walk for the self type
   or trait arguments. Normalized retained header syntax is selected directly.
   New controls check actual trace keys before the full head demand and use
   unchanged borrowed-slice/generic impls. The ordinary full head query still
   prepares its own Types, as required when actually requested.
6. No repeated runtime planning. New immutable ImplTraitIdentity and
   ImplTraitCatalog products are normal pure-query memos (stable tags65/66),
   source-bound by the same inventory key. No callbacks expand macros, read IO,
   execute guest code or prepare runtime bodies. Whole catalog membership walks
   actual raw headers once on a cold query; each identity is memoized separately.
   Typed publication consumes those facts instead of reclassifying source.
7. No new full-world namespace guard on dispatch. Cold catalog membership is
   O(N raw headers), required for complete positive/negative candidate membership;
   this is not installed operation work. `matches_catalog` is key/domain and Arc
   identity, not namespace-map construction/comparison. Each typed bucket uses
   an indexed trait lookup and its exact candidates; demanded translated impls
   have canonical/raw/source rows checked. Old whole-catalog typed cardinality
   and duplicate all-source attachment scan are removed. Selective future recipe
   invalidation/root-goal dependency binding remains an integration requirement,
   not measured by these source tests. MA007's macro catalog audit remains a
   distinct obligation; new membership does not select macro execution semantics.
8. No BodyIndex copies or whole-file searches. Parsed source and query products
   remain retained immutable Arcs. Metadata observation retains source/header/
   context inputs; exact syntax_at uses the existing node locator. Candidate maps
   own scalar IDs and Arc handles. `DefinitionReceipt::observe` also captures
   actual trait declaration kind at its exact source location. No new resource
   accounting, work/fuel/count/byte limits, sandbox or static checker.
   MA008's macro-only index/verifier finding is separate; no such path is added.

Replacement/completeness: PreparedFacts now requires the actual memo-owned source
membership catalog for the same immutable global publication. Pending membership,
partial Scope inventories or foreign worlds cannot certify absence. The new
`require_translation` source-ID half is called by actual PreparedProvider::publish;
it is not an unused test helper. It requires every member of a requested trait
bucket, with actual raw/canonical identities. Typed maps must still be one-to-one,
all supplied impl metadata must match the source classification and actual receipt,
trait rows must identify real parsed traits, and their ordered typed candidate
sets must equal the complete source sets. Additional translated impls can serve
other demanded traits/inherent items. Relevant-impl simplification/index traversal
remains the unchanged pinned upstream implementation.

Costs: initial cold identity classification visits all raw headers and needed
name/trait-declaration facts; it does not prepare all impl Types/predicates. Each
new demanded typed publication visits its own translated definitions/impls and
requested trait buckets. Header/source equality may cost O(header tokens).
Membership and classification maps are ordered and stable-encoded; no execution
hot-path/dispatch change, speed/scaling or memory reduction claim.

Validation scope and holds: V312 failed on two missing exhaustive routing matches;
V313 built and passed the updated32-head identity/full-head equivalence probe.
V314 built and passed6/6controls: entire997-header catalog membership is complete,
unknown membership cannot certify any bucket, borrowed/generic identities2/2in
all4editions,32-head equivalence, existing generated currentness and unchanged
local main. V315 failed only the new foreign-world control's attempted private
Seal key mutation; V316 uses a real independently prepared foreign publication.
Preserve all cohorts/fingerprints; do not relabel earlier results as final.

The new source-ID subset control is not an instantiated typed PreparedFacts,
ReadyProvider, inference delegate or root-goal execution. Typed definition/argument/
binder/ParamEnv/predicate/layout/lang-item translation and transitive dependency
closure are still absent. In particular ready() must not be used to claim arbitrary
unprepared callback facts are available; actual factory/closure completion and
source/epoch/currentness at goal entry are required before evaluation. There is
no interpreter-backed use of the upstream root solver yet. No new distinct stock
runtime gain, refreshed suite total, task/interpreter/JIT or Linux acceptance.

The final source-ID translation control uses the exact require_translation method called by PreparedProvider::publish, not a parallel fake publication path. It proves source membership completeness for one actual Foo candidate out of995rawheaders in each edition; it does not construct upstream typed IR or invoke publish()/ready(). A real foreign source publication is supplied to the negative control. New query key/value tags65/66 are appended without reusing existing tags; existing key-routing exhaustiveness is updated as declaration-only, with no new byte accounting.

Final V316 collected terminals: library27921/0,CLI89194/0,controls81675/0;8/8controls pass. Source/HEAD/test/CLI guards match the reviewed c8dda207 fingerprint. Complete997raw memberships/no pending, Foo1/995all4editions source-ID guard, unknown-no-absence, borrowed/generic2/2all4,32-head equivalence, generated input/epoch, unchanged local main and12Copy origin worlds pass. These are the exact executed scopes; actual typed-context publication and upstream solving remain unexecuted. Earlier compile/control cohorts retain their own fingerprints. Plan/spec/source checkpoint checks are separate document/source hygiene evidence.

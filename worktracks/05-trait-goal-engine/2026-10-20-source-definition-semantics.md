# M020 demanded definition scalar semantics

Before edits: baseline `1084e893d0`, LOCAL-049 / WT05-02. Worker owns new
`traits/upstream/source_definition_semantics.rs` and this record. Lead registers
Q83 SourceDefinitionSemantics keyed by DefinitionHeaderKey and owns prepared.rs,
source fact assembly and disposable upstream-root suspension. No Cargo/tests,
commits or subagents by this worker.

Invariant and witness: repr128.rs's actual DiscriminantKind::Discriminant and
PartialEq obligation requires real source facts; an eager complete DefinitionFacts
row must not invent false/None/empty values for unrelated missing queries. Reuse
the exact pinned next solver and distinguish each compiler query's genuine domain,
source-proven answer and missing producer. Existing custom operator/trait routes
remain held; this component does not close a runtime/root gate.

Common route: completed original/generated source -> DefinitionReceipt and
DefinitionHeaderPlan -> retained DeclarationGenerics/actual AST and real associated
parent -> Q83 immutable scalar availability -> lead's source fact admission.
Neither source queries nor AST walks belong inside solver callbacks. Real source
headers, edition, scope, canonical/aux parent attachment and immutable parser roots
remain retained through the existing memo dependency closure. Parent headers are
demanded only for semantic branches that inherit a value; lexical scope owner
digests are never treated as compiler module/parent DefIds.

SourceSemantic<T> exposes Known(T), NotApplicable (upstream invalid query) and
Pending(typed dependency) independently per query. requires_sized_self needs full
clauses/elaboration and authoritative Sized identity; opaque closure needs actual
type/body identity producers; const_conditions needs its real condition lowering.
They stay explicit dependencies. Explicit associated source aliases are genuine
ordinary declarations rather than synthetic RPITIT definitions. No anonymous ID,
default false, unvisited-body empty list or complete provider is manufactured.

Exact pinned checkout revision is ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
Inspected compiler sources: rustc_middle/src/ty/mod.rs::is_const_fn (2201),
is_conditionally_const (2215), opt_rpitit_info (1750);
rustc_const_eval/src/const_eval/fn_queries.rs::constness (11);
rustc_ast_lowering/src/item.rs::lower_constness (1664), trait-item has_value
lowering (813 onward), impl-item has_value (1065);
rustc_middle/src/ty/context/impl_interner.rs::closure_is_const (435),
alias_has_const_conditions (440), has_target_features (473), has_item_definition
(555), opaque_types_and_coroutines_defined_by (651);
rustc_ty_utils/src/ty.rs::defaultness (80);
rustc_trait_selection/src/traits/dyn_compatibility.rs::generics_require_sized_self
(317); rustc_hir_analysis/src/collect/clauses_of.rs::const_conditions (1053);
rustc_ty_utils/src/opaque_types.rs::opaque_types_defined_by (315);
rustc_codegen_ssa/src/codegen_attrs.rs::codegen_fn_attrs (563), builtin target
feature handling (102) and closure inheritance (326). Unsupported semantic branches
are held rather than replaced with a reduced independent algorithm.

Cold work is bounded by the demanded declaration and needed actual parent header,
with retained Arc query/parser metadata. No parser/evaluator/solver is introduced;
no body/signature lowering, source-world scan, hot dispatch planning or deep body
copy is added. All eight recurring architecture modes need source review at the
final fingerprint. Component evidence must use actual source identities and retain
pending/invalid-domain distinctions. Root/runtime acceptance, defaults/elaboration,
opaque and coroutine synthesis and full metadata production remain separate holds.

Implemented bounded API: SourceDefinitionSemanticsKey aliases DefinitionHeaderKey;
PreparedSourceDefinitionSemantics::prepare is Q83's pure producer. boolean takes
DefinitionBooleanQuery and returns an independent SourceSemantic<bool> for each
named scalar query. parent returns a real associated declaration Digest or a typed
ParentIdentity/Parent dependency. CurrentSourceDefinitionSemantics retains the memo
Arc and prepared input guard. alias_domain describes only actual free/projection/
inherent source alias identity; alias_argument_layouts remains a separate typed
dependency. const_conditions and both opaque-list interfaces distinguish applicable
but unprepared queries from invalid domains and never return guessed empty lists.

Constness uses direct retained AST keyword attachment and active source attributes,
with exact parent trait/trait-impl/inherent-impl inheritance. The source header's
attributes already pass outline::attributes_from_meta/effective_metas; this adapter
does not decode cfg_attr or construct another attribute parser. RustcComptime maps
to Always even together with written const: the pinned diagnostic does not change
the query value, and the interpreter does not add that static check. Conditional
trait impls consult the actual implemented trait's Q83 result. Associated trait-impl
method conditionality still needs genuine trait_item_of identity when the parent's
condition is true; it remains TraitItemIdentity pending. Ordinary explicit source
declarations cannot be compiler-synthesized RPITITs, so their genuine negative
is_impl_trait_in_trait result does not fabricate any missing anonymous identity.

has_item_definition follows defaultness.has_value. Trait associated type/default
const/default method presence uses actual retained source attachments; trait-impl
members have_value=true exactly as lower_impl_item records. Inherent items are an
invalid defaultness query, not an invented false answer. has_target_features is
applicable to actual functions and statics; absent active target_feature attributes
prove empty target_features for these non-closure declarations. Present attributes
need compiler target-feature decoding and remain pending. No closure or synthetic
coroutine identity is invented. Sized/elaborated predicates, compiler const-condition
lowering, opaque/type/body walks, nested-body identities and alias argument layouts
stay named dependencies. A source lexical scope never supplies a guessed module ID.

Independent source reviewer corrected two draft issues before freeze: Static is a
valid codegen_fn_attrs domain; and lower_constness always retains RustcComptime's
Always value despite a const keyword diagnostic. Both changes are included in the
final fingerprint. No tests/builds were run; existing official repr128/associated
sources remain the intended consumers through lead's real fact assembler and root
admission, not a new fixture-only provider.

Separate eight-mode source review: (1) no expression/assertion evaluator or solver;
(2) no scalar/break-only macro route; (3) no method/numeric/fixture-name dispatch;
(4) actual HeaderPlan/DeclarationGenerics receipts join original and generated
source, owning editions and canonical/aux parents; (5) only direct declared header,
default-presence attachments and actual parent metadata are read, no executable
body/signature preparation or unselected branch checking; (6) Q83 retains independent
memoized source/parent/implemented-trait Arcs and direct AST locations, no parsing or
planning inside root callbacks; (7) no namespace census/new invalidation policy or
hot scan; (8) no BodyIndex copy, whole-file syntax search, unsafe/resource accounting.
Pending data remains outside published completed facts; lead owns typed demand and
disposable-root suspension. Source review alone does not establish runtime safety,
actual root entry, suspension or any official runtime contract pass.

Reference source SHA256s (actual pinned checkout has no diff for these paths):

- rustc_const_eval/src/const_eval/fn_queries.rs
  `4b0651ff3991d5a62132a936e00bd272ec575dfd3c7b48c7762ad51ec1150a92`
- rustc_ast_lowering/src/item.rs
  `4726eb0e6cb374dcd56a6e6fab6ed786d0181c8bce149d989771841c88375050`
- rustc_ty_utils/src/ty.rs
  `0d1edae3ada2b3186448a44c151a6d07916461b781f1803edb9f1e5f6bd5afee`
- rustc_codegen_ssa/src/codegen_attrs.rs
  `e3df29393d8324086af70463480f6c2358f99a811473be9aaaa42e68865c953a`
- rustc_ty_utils/src/opaque_types.rs
  `67b2235cb567f255330d8f50642ee48dd5af173372a085e54ccd33ec1cd9ccd6`
- rustc_hir_analysis/src/collect/clauses_of.rs
  `465f5dd0f397f87716aedc65048a12dce7ec3af51d88e584ea5ad902b2251bf3`
- rustc_middle/src/ty/context/impl_interner.rs
  `9df7d014fea7d44e4d8e09022fc94ccb1356fff57a78ff697250e3c7a6b0e920`
- rustc_trait_selection/src/traits/dyn_compatibility.rs
  `1f7745a95de00780fe6e87972ecb15cac38377b3613805e33972c458ffbf4581`

These are cited reference locations, not newly vendored mechanically compiled
algorithm inputs. No additional originals/bootstrap/manifest edits were made;
the pinned Git object reconstructs them under tools/validation/README.md. Existing
ty/mod.rs and ty_utils/ty.rs retained inputs remain unchanged. Full upstream
elaboration/defaultness/opaque collectors are not claimed ported by this adapter.

Frozen production SHA256: source_definition_semantics.rs
`6a9d04b7e75595827042db62b3c869508d7babeccab49c751438ce6eac042626`.
rustfmt +1.99.0 and git diff --check succeeded. All source/runtime controls and
remaining compiler branches are unexecuted by this worker. Lead registers Q83 and
retains responsibility for admission, root invocation and actual terminal evidence.

Final review precision: answers remain independently available, but the Q83 bundle
currently reads its actual parent Q83 for every associated declaration. This is
conservative cold metadata demand, not evidence that each individual scalar makes
the minimum possible parent demand. Parent predicates/signatures/bodies remain
cold. The root reader must distinguish NotApplicable (invalid compiler query)
from Pending and Known(false); no missing query becomes a negative answer.

# M019/M020 — actual typed generics for the shared upstream client

Solo /root, LOCAL-049. Baseline c388c7754cb28c974627058cc6852b07e86b1ada.
Invariant: SourceFacts::generics_of must expose actual source parameters, upstream
positions, parent count, early regions and declared const parameter Types before
complete DefinitionFacts can be frozen. Current construction metadata proves only
namespace/arity; reading generics_of still requires the facts it helps construct.
Do not fill that cycle with dummy facts, guessed usize/region Types, invented
parameter IDs, fake providers or a ready token. Root solving remains unfinished.

Use one parameter-order transport for real signatures, impl heads and actual
DeclarationGenerics query products. Preserve existing signature layout/pending
function-region behavior and remove duplicated order/kind mapping. A real trait's
implicit Self is source-owned and upstream slot0 (pinned generics_of.rs:237-260),
not a synthetic function parameter. Preserve written source ordinals separately.

Add a normal pure DeclarationGenericTypes query keyed by actual catalog and
source definition. Retain DeclarationGenerics plus real source/header/context/
parser; locate const parameter Type syntax directly and resolve it through the
existing WrittenTypeDeclaration/resolve_actual_type and actual completed source
environment. No token-text parser, primitive-name-only const-type guess, body
inference, source IO or producer execution. Missing binders/projections/function
metadata remain explicit pending outcomes, never semantic NoSolution.

Retain only demanded generic query products and their referenced declaration
closure in SourceGenericLayouts. Reuse the same source-term encoder with an
actual declaration parameter frame for const Types; no second Type converter.
Materialize a context-owned immutable generic-fact table before full facts, then
serve the actual upstream generics_of/identity-argument constructors from it.
Full publication must compare exact prepared generics (parameter identity/index,
kind, parent count and const Type), not only kind/count. Typed generic bootstrap
is not full facts, ReadyProvider, an inference delegate or root evaluation.

Primary witnesses: unchanged existing impl/trait/default matrices, all editions
and original/user macro/wrapper; const-generics/type-after-const-ok.rs and
type-dependent/issue-61936.rs actual nominal/trait/impl declarations, preserving
const/Type/region ordering and declared usize. Existing real signature-order and
pending late-region controls must retain their meanings. Supplemental checks
inspect actual shared kinds/IDs, memo reuse, no executable/source query during
materialization, stale/foreign root guards and cleanup; imported originals remain
unchanged/licensed. Per-declaration/transitive currentness closure and parameter
DefId publication stay explicit readiness obligations until actual integration.

Cold cost: memoized declaration/const-Type preparation and demanded referenced
metadata closure once; context-local immutable list interning once; callbacks
borrow actual facts through relevant-ID lookup. No installed operation parsing,
plan reconstruction, world namespace scan, BodyIndex copying, accounting or caps.
Review all8 recurring modes plus separate MA004/MA007/MA008 before source checkpoint;
retain exact sources/async terminals/failed attempts. No new stock runtime gain or
full M019/M020/interpreter/JIT/Linux/performance acceptance follows from data alone.

V330 source witness: nominal A<const N:usize,T> and Foo<const B:bool> plus
its real generic function supply actual typed generics. Inherited next's const
parameter Type remains pending in issue-61936. The occurrence belongs to its
impl's generic header, outside the child function's declaration range. Resolve
inherited Type syntax from the genuine parent DeclarationGenerics/source scope,
not through the child's lexical-name receipt and not via primitive-name fallback.
Validate actual own/inherited layout and parent metadata. Rebase declared parent
parameter keys to the child's captured source ordinals at the existing ordinary
Type/const parameter leaves; do not add a tree substitution evaluator or synthetic
signature. Preserve the original scoped source owner/hygiene/edition for lookup.
V33014/15controls and exact sources remain preserved, not relabeled as final.

V331 failed probe: captured next layout is own0/inherited3, but scope.owner is
the impl member-container identity, not its canonical declaration. Extend the
existing cold PreparedScopeImplHeaders original associated-owner join with a
scoped member-container -> canonical declaration index. GeneratedSyntaxInput
retains the analogous map from its actual admitted Outline/GeneratedHeader join;
read it through the existing GeneratedSyntax input dependency. This replaces the
wrong direct owner-as-definition query. Both routes validate the real parent
header/context and share ordinary inherited Type lookup/rebasing; no runtime
lookup, namespace census, parser, guessed primitive Type or ready solver token.

Final V333: both builds and15/15controls pass with final guards. Original/generated scoped parent joins share the same ordinary inherited Type path. Twenty actual metadata worlds across all editions cover48rows/36consts/24early regions. Zero new distinct runtime gain, full fact/inference/solver/milestone/JIT completion; see worktracks/evidence/2026-10-19-v328-v333-typed-source-generics.json and the separate source review.

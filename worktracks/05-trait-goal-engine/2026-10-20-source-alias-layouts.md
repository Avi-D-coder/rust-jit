# M020 / LOCAL-049 — prepare actual alias argument layouts before entry

Invariant: an upstream alias constructor checks the argument namespaces of its
actual source declaration. It must not discover a missing layout inside an
abort-profile solver callback or substitute a same-spelled declaration. Minimal
unchanged witness: `generic-associated-types/streaming_iterator.rs`, whose trait
`Item<'a>` owns a different generic identity/layout from its implementing alias.
The reached `enum-discriminant/repr128.rs` projection additionally needs the
actual DiscriminantKind associated declaration, not a member-name digest.

Shared route: admitted ordinary/generated ParseFile -> DeclarationGenerics ->
DefinitionHeaderPlan/Q75 and SourceDefinitionSemantics/Q83 -> common
DeclarationParameters parent-first mapping. Trait-associated type identity
continues through TraitAssociatedTypes/Q77. Add a separate immutable memo demand
for alias layout, so scalar/header queries do not prepare this extra closure.
Runtime remains the ordinary source/member/VM route. No new parser, evaluator,
type solver, body lowering or runtime source query is introduced.

Pinned policy: rustc_middle/src/ty/context.rs:2162–2230; projection/free/impl-form
aliases use all parent-first arguments; inherent type/self-form const aliases
use Self followed by own arguments. Reuse DeclarationParameters rather than
creating a second parameter mapper. Associated constants need genuine member
identity support; opaque/anonymous identities remain explicit gaps.

Lead owns source_alias_layouts.rs, query registration, the existing GAT source
control and integration glue. Worker trait and impl semantic modules are
disjoint. Cache dependencies retain actual source, scope, edition, expansion
epoch and relevant Q77 member inputs; currentness uses PreparedInputGuard.
Preparation is cold O(parameters + selected source/member dependencies), then
immutable Arc retention; installed runtime operations do not plan source.

Acceptance required: unchanged GAT source across four editions and three
origins, exact parent/own namespaces and IDs, memo reuse, body-cold trace and
epoch withdrawal; genuine pending domains remain pending. This source component
does not certify complete callbacks, a solver root or runtime compatibility.
The old DefinitionFacts.alias_layouts construction input remains held until a
real fact factory consumes the authenticated layouts; no old path is called
complete by introducing this plan.

Eight-mode review is required against the final fingerprint: inspect ordinary
source joins (1–4), no pattern/arm preparation (5), memo and callback boundary
(6), selected Q77 buckets/currentness rather than runtime world scans (7), and
retained AST/Arc handles/direct syntax locations (8). MA001–MA008 are separate
historical audit findings; this bounded work does not close them.

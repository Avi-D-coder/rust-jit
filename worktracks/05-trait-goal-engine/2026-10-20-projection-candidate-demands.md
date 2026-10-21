# M020 / LOCAL-049: projection-parent candidate demands

Invariant: a demanded input projection must retain its actual parent trait's complete source candidate bucket before pinned normalization. The unchanged repr128.rs source bound `<T as DiscriminantKind>::Discriminant: PartialEq<U>` is the minimal witness. Pinned normalizes_to::compute_normalizes_to_goal probes the parent trait and ordinary assembly reads relevant impls even after a builtin candidate succeeds. Outer PartialEq membership alone is insufficient.

Extend SourceFactBundle::prepare after existing SourceBindings::prepare authenticates actual source input types and their Q77 associated identities. Merge the retained projection-parent source keys into the request's candidate-read set before the existing complete membership/head capture loop. Do not grow a semantic solver or read every trait mentioned by unrelated definition metadata. Existing unresolved/broken projection identities remain typed pending and cannot certify absence. Nested projections use the ordinary require_type traversal. All candidate rows remain present; genuine complete empty buckets are known empty. No runtime/CLI/root activation is part of this slice.

The ordinary/generated syntax join stays DeclarationSourcePredicates -> WrittenTypeDeclaration -> TyTerm::Projection -> SourceBindings::prepare -> retained Q77/DeclarationGenericTypes -> shared source factory -> CandidateIndex. No alternate route is introduced or removed; the incomplete outer-only request is replaced only for actual demanded input projections. Source/header/edition/epoch dependencies stay in retained query guards, with currentness and fresh withdrawal controls.

Cold cost: ordered insertion per actual distinct projection parent plus existing complete candidate/head preparation for newly demanded buckets; immutable memo values are shared. Installed dispatch receives no parsing, body preparation, global namespace scan or BodyIndex copies. All eight architecture modes reviewed: no separate macro/assertion evaluator, generated scalar path, context-only method/numeric rule, origin exclusion, pattern engine/cold executable work, per-entry replanning, global namespace guard or immutable body copy. Exact source fingerprint and execution evidence remain pending.

Control: retain unchanged owning-2015 repr128 source and existing full outer66-candidate assertion. Obtain projected subjects through actual DeclarationSourcePredicates; use only authentic resolved source recipes, assert parent identity against actual source declarations, compare requested membership to the actual catalog (including empty parent), and retain actual memo/cold/currentness/withdrawal checks. No original main/solver-root/full-fixture pass follows from component success. Existing delegation parser failure remains selected and held, never filtered away.

The actual selected-source IR join also needs these parent identities after relation/numeric selection, rather than re-reading all callee clauses. SourceRuntimeDemands::candidate_traits checks its existing same-owner/snapshot contract and walks only retained NumericSourceClause values using clause_candidate_traits. The latter uses actual ClauseKind::Trait IDs and TypeVisitor projection AliasTy owners; source_projection_parent reads immutable SourceGenericLayouts construction metadata, with missing mapping held as ProjectionParent(actual associated DefinitionId). It never normalizes or selects impls, and returns owned source IDs suitable for later coordinator preparation outside the nonescaping context. Other callback families, future-use numeric closure, const/opaque semantic demands and exact reached-call consumption remain unimplemented; this set is structural reads, never a complete readiness certificate. Existing Q80 projected-source encoding control now checks exact outer+parent IDs on its real clause in all selected edition/origin worlds. No fake WF goal is introduced.

Independent source review, 2026-10-20 (source component only): no actionable
finding in the bounded additions. Exact SHA256 reviewed:
- `source_fact_bundle.rs`: `f52ac7e9bf099c0829991577e9ed0919972d919670ccbf1440e183e0473093d6`.
- `source_factory.rs`: `5187810e5089ce261367832a85d1dbf51764625904f6d1a118f3b1903e615ecf`.
- `source_runtime_demands.rs`: `883dca755df22666bf4092ef3af8db630ee1c9693221d660bfc4af88dc0dff46`.
- `source_predicates.rs`: `a78cbbd302a6a5f4ba6f4d0010affc39e918c3469a2d28fe4e3e9ff6cbcda5e2`.

`SourceFactBundle::prepare` derives additional parent demands from actual
`SourceBindings::projections`, after ordinary `require_type` traversal and Q77
identity authentication. Missing binding metadata remains pending; no lexical
trait selection or absence-as-empty is added. Every demanded trait still uses
the complete actual Q65 raw/canonical rows. `candidate_traits` rechecks the
retained inference owner/snapshot; `clause_candidate_traits` reads actual outer
Trait clauses and recursively visits unchanged TypeIR projection Types. Each
projection parent is read from the authenticated `SourceGenericLayouts` map,
and every exported parent/outer ID is checked against the context source world.
The new `check_actual_projected_source_predicates` assertion examines the real
encoded Q80 clause and compares its structural reads to the actual outer and
associated-parent identities. `check_source_factory` observes actual Known
membership rows before the same production `PreparedProvider::publish`; the
control compares exact sorted canonical buckets to actual Q65 membership.

Separate eight-mode review of these additions:
1. No assertion/expression evaluator: only existing source bindings and TypeIR
   metadata are read; the assertion is a control of the production helper.
2. No literal/break macro-output route: ordinary Q80/Q77 source identities and
   the existing shared encoder supply original/admitted generated syntax.
3. No method or numeric solver: the visitor exports source trait identities;
   it does not match candidates, normalize aliases or choose primitive widths.
4. No origin exclusion: missing source metadata produces the existing typed
   pending path regardless of original/generated origin.
5. No patterns or unselected executable arms: preparation touches demanded
   declaration metadata and complete candidate headers only.
6. No installed parsing/replanning: cold binding preparation is retained behind
   exact memo guards; runtime demand metadata visits retained selected clauses.
7. No new full-world dispatch/guard scan: parent insertion is ordered per actual
   projection; candidate enumeration uses relevant complete catalog buckets.
8. No immutable body copies or whole-file node searches: retained query/IR
   handles and the ordinary recursive TypeVisitor are used.

Cost is linear in visited selected TypeIR structure plus ordered distinct-ID
insertion and the existing cold preparation cost of newly demanded complete
buckets. This is structural source evidence, not measured performance. The
visitor is deliberately not a complete source callback/readiness closure:
const/opaque and subsequent solver demands, demand-filtered source fragments,
ordinary source coordinator continuation and actual reached-root/VM consumption
remain held. No Cargo/test or production edit was performed by this reviewer;
the new assertions, component publication and unchanged repr128 runtime remain
unexecuted in this review.

Successor recheck: `source_runtime_demands.rs` SHA256
`863e9d86595f00979a15d6133e5a2eff7091ca95b0081eee2909f5dadb47579d`
supersedes only the runtime-demand hash above. The other three hashes are
unchanged. `select` now builds the structural candidate set once after actual
numeric incidence selection, before constructing `SourceRuntimeDemands`, and
retains it privately. `candidate_traits` returns a borrowed immutable set after
the same owner/snapshot check; it no longer revisits TypeIR on each metadata
read. A missing projection parent fails selection with the same typed pending
value rather than installing a partial set. All eight conclusions above remain
valid; mode 6 is strengthened by retained phase selection. The source walk cost
occurs once per demand-plan selection and subsequent candidate-set reads perform
only the existing owner/snapshot guard. This successor was source-reviewed only;
no Cargo, test or runtime acceptance is claimed.

Before validation, the selected-source join retains its BTreeSet once during SourceRuntimeDemands::select; candidate_traits only checks the actual owner/snapshot and borrows that immutable set. Repeated phase reads therefore do no TypeIR demand traversal. This successor needs its own final source fingerprint; prior review hashes stay historical.

## Lead prevalidation source fingerprint review

`crates/lrr/src/expand/source_discovery.rs`: `7a9b5b5ea7da8c5c8cfa401150665639d460949c6528e3c9459ef6b6d50fc428`.

`crates/lrr/src/expand/source_discovery/compiler_preparation.rs`: `1f883613f3299d510f3bebc8c02d149dff1d88a24cf515b86019ebee4c194363`.

`crates/lrr/src/traits/upstream/source_fact_bundle.rs`: `f52ac7e9bf099c0829991577e9ed0919972d919670ccbf1440e183e0473093d6`.

`crates/lrr/src/traits/upstream/source_factory.rs`: `5187810e5089ce261367832a85d1dbf51764625904f6d1a118f3b1903e615ecf`.

`crates/lrr/src/traits/upstream/source_runtime_demands.rs`: `863e9d86595f00979a15d6133e5a2eff7091ca95b0081eee2909f5dadb47579d`.

`crates/lrr/src/traits/upstream/source_impl_semantics.rs`: `1e339c1f540a846b3f0d65297baa088604bce1fe50f0fa3c270376061499a924`.

`crates/lrr/src/traits/upstream/source_impl_clauses.rs`: `3c8136abc57eff5fe3dd0d461f4623e3072b08a6bdd804d21718c8fbdabacc7a`.

`crates/lrr/src/traits/upstream/source_declaration_clauses.rs`: `cd801eae04fb7eae8daed1ff7ee870abd0e294b59e903ba97ce56852eefe0513`.

`crates/lrr/src/cli/preentry_loader_tests/upstream_projection_types.rs`: `c3328c3a5561d39b8e0254db2cb203007df4f1e334874fb6ca0f95e2ad62e77a`.

The implementing lead separately reviewed prepare_declarative/resume_compiler_preparation/finish_compiler_preparation and prepare_install_macro_routes: actual provider/profile/environment/procedural state is retained once, incomplete configuration cannot poll or publish, and the route cache key is recomputed after module adoption. Typed NeedModule is not a runnable graph. Immediate strict constructors preserve their explicit immediate-ready contract. Exact physical module/service continuation is still absent; this source refactor alone supplies no resumed guest call. Cold route-vector comparisons do not run in installed guest dispatch. Existing global namespace or legacy alternate runtime routes are not certified removed by this change.

The full-clause join captures genuine known full-plan bindings and their query guards before layout construction, validates exact canonical impl identity during encoding, and shares one immutable complete list between all/own (impl has no parent). There is no fallback from full-plan failure to a guessed empty list. The observer counts genuinely empty rows separately from all complete rows and checks exact canonical membership. Shared owned-ordinal recipe encoding uses retained source/emission; it adds neither parsing nor an alternate clause emitter. Projection dependency traversal is retained once in SourceRuntimeDemands, with only guarded borrowed-set reads afterward. No installed VM path is activated.

All eight modes: no assertion/generated evaluator or second parser (1,2); no macro/context-selected inference/member rule (3); no new generated-source exclusion (4); no pattern/cold executable preparation (5); source queries/memo handles and retained plans, no installed planning (6); demanded bucket preparation and exact query guards, no dispatch namespace census (7); immutable Arc handles and direct retained syntax locations, no BodyIndex deep copies (8). Current sources await actual V531 build/control terminals; component, root, runtime and full-fixture outcomes remain separate.

## Independent reviewer V531 prevalidation pass

The reviewer separately inspected the projection candidate diff at bundle
`edc99df1829ea5d38b9cba3972eec1c275f425bf8bbd523f7c21d8e036a4caae`,
factory `223eb4b433558d0f84a3b89e078f6a21affde620ac469200468a597068a8253e`,
and control `c22036260e95f0ed7fd57215738e75b6a63c6d1ef2cd44221ca6a1d2efe43a54`
before the full-clause successor. InputBindings' genuine Q77 joins alone extend
the candidate-read set; complete Q65 membership is retained including empty
buckets. The test obtains the actual projected subject from Q80, retains both
enum inputs, compares exact Known bucket maps/full canonical union, and retains
currentness/memo/cold/withdrawal. The source-only structural candidate visitor
now retained at `863e9d86595f00979a15d6133e5a2eff7091ca95b0081eee2909f5dadb47579d`
checks actual same-owner selected clauses, actual source-world IDs and immutable
projection-parent metadata; missing parent is typed pending. No normalization or
solver callback is invoked. Repeated accessor reads borrow the cached ID set.
Direct projection clauses/const/opaque paths and demand-filtered whole-Q80
admission remain outside this component evidence. Nested structural controls
are unexecuted; old V530 nominal-only bucket success remains tied to that source.

Final module continuation sources7a9b5b5e.../1f883613... were independently
read: exact compiler/proc Arc and env/engine identities survive repeats; cached
route+demand identity includes actual provider/module/source/inert inputs;
consuming resume invalidates stale demand after adoption and recomputes graph
cache keys. One finalizer requires actual namespace installation/nondeferred
imports; incomplete configuration cannot poll/publish or mutate late facets.
Strict ordinary constructors still require immediate Ready. Full-impl sources
1e339c1f.../3c8136ab... and owned-ordinal accessorcd801eae... were independently
read: actual Q80/Q84 whole-parameter implicit sites distinguish nested written
bounds, parameter order interleaves genuine ConstArgHasType rows with exact
source key/Type/span, and pair-dedup retains first source order. Projection,
empty-where, default-impl, GCE and attribute gaps stay typed. Failed full proof
cannot fall back to empty. The actual2-fixture CLI control selects genuine
Q65/local impl identity, asserts ordered kind/parameter/type/span observations
and retains Q86 memo/currentness/cold/withdrawal guards. No actionable source
finding in this bounded pass; mechanical compilation remains unexecuted.

All eight modes independently checked against actual changed entry points:
1/2 no assertion/generated executor or second parser;3 no macro/context numeric
or member rule;4 no generated-origin exclusion;5 no pattern/body/unselected
branch preparation;6 retained source queries/phase state, no installed source
planning;7 exact demanded buckets/guards, no dispatch namespace census;8 shared
Arc plans and exact retained syntax, no BodyIndex copies or whole-file lookup.
Physical module/service continuation, live root/response and runtime acceptance
remain held. Current aggregate fingerprint will be recorded at fresh capture.

V531 plan: direct Rust1.99 offline library no-run JSON, CLI and digest helper
builds sequentially under sole Cargo ownership; after-build source/HEAD/binary
guard, actual-list dry-run, exactly seven maintained selectors, then source/
HEAD/test/CLI and independent digest closing guards. Preserve the known failing
delegation original/control. Stop dependent stages after any compile error.
Write compact durable commands/outcomes/hash capsule before run-owned fixture
cleanup. No unchanged runtime replay and no broad redundant tests.

V531 terminal fingerprint5978864c4b77fb5b0581f88cffd0d8bfe5b5edd8ba93aa20a7ba908f01a8dc0f
(8749 inputs, HEAD680653d11761587c878b368d6b95f9879b1ee5ca) failed
library build101: source_impl_clauses.rs148 E0599 needs AST HasTypeBounds
import for WherePred.type_bound_list. CLI/DG/all7 controls remain unexecuted;
no fresh binaries or acceptance inferred. Source-only closing guard passed0
with unchanged source/HEAD. [Durable V531 compile capsule](../evidence/2026-10-20-v531-full-impl-compile.json)
retains exact command, full error and raw hashes. No fixture projects/private
targets/copies were created. All launched commands terminal, evidence recorded
and ownership released before mechanical source repair.

V532 prevalidation: exact manifest diff from V531 is only source_impl_clauses.rs,
adding the existing AST HasTypeBounds trait import. No semantic/assertion/
selection change. Captured source9281e78871cc15ac2a9570b86f089a1490c9342514a1b508414ce793368ac955
(8749 inputs, unchanged HEAD680653d11761587c878b368d6b95f9879b1ee5ca).
Same seven exact controls remain selected, including known unsupported delegation
original; no runtime replay. Fresh build/terminal evidence is pending.

## V532 actual terminal evidence

Source9281e78871cc15ac2a9570b86f089a1490c9342514a1b508414ce793368ac955
/8749/HEAD680653d11761587c878b368d6b95f9879b1ee5ca: all3 builds0
(lib46.44s,CLI37.01s,DG0.63s). All7 exact selectors matched one test body
and unchanged inputs. Six pass; the retained anonymous control fails101 only
at unchanged infer-defaults.rs reuse declaration parse353..353 after completing
the prior two source worlds. No test/fixture filtering or assertion weakening.

Factory successor passes exact two buckets (DiscriminantKind genuine empty,
PartialEq66),103defs/10traits/66impls,28empty and66complete impl clause lists,
66derived outlives,24ADTs/7enum reprs,115explicit remaining encoding holds.
All enum/current/memo/cold/withdrawal/coarse-root-hold assertions complete.
Unsafe-C alias positive plus separate erased-header hold pass across4 editions.
Projected source predicates pass12worlds (4editions x original/user/wrapper),
36own clauses/12projected subjects, exact structural outer+parent IDs and
existing owner/IR/memo/currentness controls. This does not execute the runtime
SourceRuntimeDemands selection/root/response pipeline. New full impl observer
passes both unchanged owning2015 fixtures: Sized(T0) before Const(N1,usize),
and Const(N0,usize) before Bar bound retaining the same N0. Actual Type/key/span,
memo/cold/currentness/withdrawal complete. Both minimal compiler module controls
pass (repeatable typed demand, unchanged auxiliary Ready/strict seal parity).
They do not prove physical module adoption/service continuation into a guest call.

After-build and closing source/HEAD/test/CLI guards pass0; independent full
digester hash matches. [V532 durable capsule](../evidence/2026-10-20-v532-source-factory-controls.json)
records exact commands/binaries/control timings/counts/all115holds/raw hashes
and limits. Four alias fixture directories were inspected against unchanged
original+external witness and removed only after capsule creation; successful
projection worlds removed their wrappers. No private targets or source snapshots
created; compact logs retained. Every command terminal and ownership returned.
No runtime gain, actual solver root, full fixture, Linux, performance or JIT
acceptance follows from these component results.

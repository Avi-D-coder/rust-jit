# Held M020 source review: ahead-of-entry trait, impl and alias inputs

Lead separate source pass, baseline `3d481c9d4eaca5326b23b9a30c1c59b7f7afcd0a`,
candidate source fingerprint
`7bb5efdf1980e4762d4fe7e0c66b5c5d250a702e71661746f4954413627de994`
(8720 inputs). This review describes source, not an executed root or runtime
acceptance. V513 has been assigned asynchronously; terminals are pending.

Task/claim: WT05-02, LOCAL-049, M020. Invariant: callback data must be prepared
from genuine admitted source before abort-profile solver entry. Neither
declaration spelling nor a missing optional value proves a solver fact. The
upstream solver remains the only solving implementation. Source metadata plans
are held components until a complete relevant query closure and the actual
runtime root consume them.

| Recurring failure mode | Actual inspected route and conclusion |
| --- | --- |
| 1 separate assertion/expression evaluator | Q85 `PreparedSourceTraitSemantics::prepare`, Q86 `PreparedSourceImplSemantics::prepare` and Q87 `PreparedSourceAliasLayouts::prepare` produce source inputs only. No assertion, expression compiler or VM dispatch branch added. |
| 2 literal/break macro executor | All three join existing Q75 `DefinitionHeaderPlan`, Q83 scalar source semantics and/or retained `TraitDeclaration`/`ImplTraitIdentity`; no macro evaluator, parser namespace or generated scalar route. |
| 3 context-specific methods/numeric inference | Q85 flags follow actual trait syntax/attributes and Q76 compiler roles/configuration; Q86 modifiers and null clauses follow actual impl header/query branches. No test, macro, method-name or numeric descriptor selection in production. Runtime literal fallback and repr128 failure have not been changed. |
| 4 generated-source exclusion | Same ParseKey/HeaderRecipe/DeclarationContext identity checks and generic/parent joins apply to original and generated declarations. Q87 authenticates trait type members with Q77, not a name digest as declaration ID. Missing associated-constant/opaque/anonymous producers remain explicit gaps; existing supported source is not rerouted. |
| 5 duplicated patterns/eager cold arms | No pattern implementation, callee-body preparation, unselected-arm requirement, borrow checker or static acceptance check added. Source flags do not authorize static-rule runtime refusals. |
| 6 repeated planning | New Q85/Q86/Q87 registrations in `db::{key,value,provider}`, CLI preparation and service expansion classification use pure memo queries. Plans retain Arc query/parser roots. `layout`, boolean/modifier accessors and `encode_head` do no source fetch/IO. `CurrentSource*::capture/require_current` use existing prepared-input guards. Per-entry factory integration is unfinished, so no hot-path speed claim. |
| 7 global scan/invalidation | New cold Q85/Q86 configuration selection scans retained environment sources for a unique owning crate root: O(source count) per cold plan. Q87 uses the existing selected trait member inventory/buckets. These scans have no installed-operation or dispatch caller. No new full-world namespace-map comparison or runtime census; broader existing catalog dependencies are not claimed narrowed by this work. |
| 8 immutable body copies/searches | No BodyIndex clone or body traversal. AST access uses retained declarations/direct `syntax_at`; metadata plans share Arc roots. Q87 calls existing `DeclarationParameters` for argument order instead of adding another mapper. |

These eight recurring modes are distinct from MA001–MA008 in the historical
audit. No historical finding is closed by this component review.

## Genuine domains and deliberate holds

Q85 distinguishes capture from per-query availability. Auto/unsafe/alias come
from actual TraitHeader; marker/coinductive/fundamental/deny flags from retained
semantic attributes. Absence of an explicit dyn-incompatible attribute does not
prove dyn compatibility. Sizedness is the compiler role Sized or MetaSized,
excluding PointeeSized; default-trait false requires actual admitted owning
configuration/no-override evidence. No invented complete TraitFacts row.

Q86 authenticates raw-to-canonical impl identity and calls the common source
head encoder. `inferred_outlives_of(Impl)` is genuinely empty by the pinned
compiler's role branch; that does not make `impl_super_outlives` empty. The
latter remains `ElaborationClosure` pending. `complete_clauses_of` can return
its privately constructed empty result only for actual zero-parameter,
nondefault impls without where predicates, relevant attribute effects or GCE;
other branches stay specifically pending. No unrelated predicate family is
populated from that result. Generic GAT impl is the non-null control.

Q87 is a separate demand from Q83's scalar domains. Ordinary/free/projection
layouts use the shared parent-first parameter map; inherent self form uses
Self plus own parameters, following pinned context.rs:2162–2230. The previous
always-pending Q83 alias-layout marker method is removed. The old
`DefinitionFacts.alias_layouts` construction input and
`SourceGenericLayouts::projection_argument_kinds` remain named migration
boundaries until an actual factory consumes authenticated layouts. Source
declaration layouts are not a complete callback-preparation certificate.

## Evidence and next execution boundary

The six selected controls and two held negative controls are enumerated in
[the selection](2026-10-20-ahead-preparation-controls.json). Extended controls
reuse unchanged repr128 and streaming_iterator originals. Their supplemental
checks inspect actual IDs, bounds, argument namespaces, null/non-null query
domains, memo identity, cold-body trace and currentness. Runtime replay still
uses complete unchanged mains; no hand-selected callee substitutes for one.

Required integration remains actual complete implied/super/item clause
producers, genuine relevant candidate fact production, per-query publication,
a closure/readiness certificate bound to the exact root/caller ParamEnv and
inference owner, owned inferred-result export and ordinary call-site binding.
Existing ReadyProvider checks provider/context only and cannot itself certify
a narrowed root closure. Missing facts must be reported before entry; no
catch_unwind missing-fact protocol is authorized or introduced. Guest unwind
semantics are separate required interpreter work.

Independent review additionally found a pinned PartialEq declaration mismatch
in the existing facade. The actual current-source query answer is not thereby
the pinned-library answer. Alignment and exact filtered implied-clause policy
remain prerequisites; see the Q85 handoff refinement. No root/factory,
architecture milestone, interpreter, JIT or full-suite gate is closed here.

## Failed build and mechanically repaired candidate

V513 lib terminal101 at the reviewed fingerprint above: Q87 referred to the
associated-name helper in `traits::member_index` instead of the existing
`frontend::member_index`; Q86 attempted StableEncode on
PreparedScopeImplHeaders instead of retaining its existing stable fingerprint.
No CLI/digester build, controls or guest runtime ran. Both pinned audits passed
(234 vendor files / 4,085,867 bytes, zero changed originals and exact Git-object
verification; unchanged default-bound algorithm/configuration audit). All
source/HEAD and explicitly old V512 binary closing guards passed. These are
failed-build/source-audit observations, not runtime evidence.

Only those two source lines changed afterward. V514 candidate fingerprint is
`1ff6d8be1242d77b67a888276b41836cf5319ea62f043cebecdaaf208cb85b70`,
8720 inputs, same baseline HEAD. This source pass rechecked the helper join and
retained-fingerprint encoding against their existing APIs; other source
conclusions above remain attached to V513 and unchanged inspected code. V514
validation is asynchronous and its execution results are pending.

## V514 terminal evidence

Validation has now closed at the V514 fingerprint above. lib, CLI and source
digester builds all exit0. Six selected controls all exit0 with one selected
body and unchanged-input guards. Observed edition/origin worlds are
12/12/12/12/12/4; the constructor controls finish 264/1624 ordinary operations.
The extended repr128 control checks actual seven-facade-trait flags, nine
associated output bounds and two genuine primitive impl query domains per
world. The GAT control checks the ordinary/generated trait domain, real
projection versus impl-alias namespace domains, and the generic impl's
non-null clause hold. These additional observations come from assertions in
the fingerprint-bound passed bodies, not additional printed trace counters.
Q86's actual super-outlives remains explicitly pending in the passing controls.

Owning-2015 unchanged runtime replay remains 3/4: associated constructor,
generic-derived type and empty brackets pass with thirteen checks each;
repr128 remains executor `LRR-E-OP-TYPE`, exit2, zero checks. Empty-bracket
warning diagnostics remain outstanding separately. Zero new distinct runtime
gain; no refreshed suite total. Both negative source controls remain explicitly
held/unexecuted. No interpreted upstream root or full milestone pass is claimed.

Closing source/HEAD/test/CLI/digester checks all match. Test binary
`8a9844a87737b538dba44804bc84e884dead0db4314218c551d5eb8922a230f7`, CLI
`57a7153a5225f34e65243eba2846ab0a2d4c43bd9c49785c40c5edb75d358268`, digester
`de0973f48eae8998ab5998fa15f00b653f05dd6bb2906ca2d03f7afa58f86b2f`.
All processes are terminal and source ownership is returned. Runtime remains
one ordinary VM path; this evidence verifies source preparation inputs and
existing runtime regressions, not the still-missing root integration.

Durable combined receipt:
[V513/V514](../evidence/2026-10-20-v513-v514-ahead-preparation.json).

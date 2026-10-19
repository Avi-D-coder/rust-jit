# M019/M020 — actual source candidate buckets before typed solver facts

Solo /root LOCAL-049, baseline fe104a8ad466384ace6468f1a23fa7c1d94ce435.
Invariant: retain the complete pinned active raw inventory, but do not require
ready Type/predicate facts for every unrelated impl before a demanded trait goal.
Never omit unknown candidate memberships or invent an empty bucket.

Minimal current source witness: PreparedProvider::publish demands
implementations.len()==impl_declarations.len()==catalog.headers.len(). The unchanged
coherence-impl-in-fn and method-self-arg-trait declarations include several traits
and inherent impls; a single trait request should not prepare unrelated self Types
or predicates. Existing32-head fixtures establish real source/head inputs, not
this new candidate completeness or any upstream root solving.

Common route: ImplHeaderSyntax -> canonical declaration metadata -> ordinary
source_types::projection_trait's source/name/trait-declaration service. Extract
its identity-only prefix (before generic arguments) and reuse it both in the old
projection client and a memoized ImplTraitIdentity query. Share the original impl
metadata observer with ImplHeadTypes. Preserve original/generated/local ownership,
editions, hygiene and actual source proof; no invented signatures or parser.

A memoized TraitImplCandidates query observes the actual complete catalog and
identity-only classification for every active raw header. It retains all pending
memberships; none may become negative evidence. Only complete membership yields
an exact raw/canonical candidate set, including negative/blanket/conditional impls.
No applicability algorithm, static coherence rejection or custom solver is added.

PreparedProvider::publish must validate each demanded typed trait bucket against
that privately prepared actual source proof and the same immutable catalog. Each
translated impl still binds its exact retained source and canonical owner. Delete
the whole-catalog typed cardinality requirement only when that replacement exists;
keep one-to-one typed metadata and exact demanded bucket completeness. Callback
facts/inference/transitive closure and actual upstream evaluation remain separate
integration obligations, not satisfied by a new query API.

Cold costs: one retained identity query per needed declaration; bucket collection
walks the complete raw inventory once, observing identity-only headers and relevant
namespace deps. No self Type/argument/predicate/body preparation merely to classify
a different trait. Repeated buckets reuse identity memos; normal dispatch adds no
world scan, source parsing, plan reconstruction or BodyIndex copies. New data is
OS-owned; no byte/work/count/fuel budget. No sandbox or static checker.

Primary witnesses: unchanged pinned rustc trait/coherence fixtures, all4editions,
real trait/default/conditional/generic contexts. Check exact source/candidate IDs,
unknown memberships, memo reuse and changed epoch/input; trace that identity-only
requests do not demand ImplHeadTypes/ImplPredicates/executable queries. Keep complete
upstream interpreter/runtime/diagnostic/tooling outcomes distinct. Review all8
recurring modes and applicable MA004/MA007/MA008 pipeline/catalog/index obligations.
Generated/alias/import/parent cases not executed remain explicit gaps. No M019/M020,
solver/interpreter/JIT closure, performance claim or native validation.

Observed V314:6/6controls pass; complete997-header global membership, borrowed/generic identities2/2all4editions without full Type/predicate queries, unknown-no-absence control and32-head equivalence. Final V316 adds the actual source-ID guard called by PreparedProvider::publish: unchanged Foo needs1/995source candidates in all4editions, with missing/changed IDs and real foreign publication refused. Actual typed-fact context/inference/root evaluation remains uninstantiated. This removes the old eager typed cardinality API without weakening complete raw membership; final results/fingerprint remain separately recorded.

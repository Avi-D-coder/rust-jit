# M020 relation fulfillment and numeric-demand source review

Read-only independent review requested by the implementing lead before the next
freeze. No production source, tools, fixtures, vendor inputs, Cargo or Git changed.
This is source review, not compile/runtime or unchanged repr128 acceptance.

Reviewed exact fingerprints:

- traits/upstream/root_relations.rs: 5e7ca4995e8742ff8a00a680cd2348c915e1d877987549124bb776c4d532cfa3
- traits/upstream/source_runtime_demands.rs: b721073cae6416b0c5ec3722f6710f4e8a06892317b0b019d21cc0996e5490fd
- traits/upstream/infer/fulfillment.rs: 03ff24c31c25e9a5e04a675296cc4afbb06b5068db5784b0b6659b9a4895fc9d
- traits/upstream/root_variables.rs: 73a88378cd44a48332e991f6d9200677cd3b38f73691c554c6087d55d5acb1a5

## Findings

No source defect found in the changed inactive fulfillment/metadata join.
RootRelationFulfillmentWork::fulfill has module-private visibility, no external
execution accessor, and no caller found by exhaustive LRR source references.
RootRelationFulfillmentOutcome::Settled and the private-field settled type cannot
be constructed by another module. Its sole constructor follows real registration
into the existing pinned FulfillmentContext, try_evaluate_obligations success,
collect_remaining_errors success, an empty error vector and no pending obligations.
No test/ready token or externally supplied completion bool exists. Both runtime
numeric-demand entry points also have no caller in current source. Thus this
component neither activates a solver root nor demonstrates runtime integration.

PreparedRootRelations::prepare checks the actual RootVariables owner and exact
specifications against retained input, uses genuine source generic layout and
caller-owned ParamEnv, and instantiates actual callee arguments. apply uses the
existing upstream relation transaction; all child/coercion obligations and their
original cause/callee/signature/relation/site/env/predicate/depth are transferred.
SourceRootApplication now retains the exact original RootVariables reference.
into_fulfillment_work checks the same SolverContext pointer and exact variables
pointer, returning the complete original parts on mismatch. Original application
and obligations are not replaceable through this work type.

Fulfillment clones each actual obligation once into the existing common upstream
engine, retaining the original ThinVec untouched. Its private execution method
checks the retained owner/variables and snapshot depth, and can register only once.
The ordinary applied view's outstanding_relation_goals count is never cleared or
decremented. after_relations still holds nonzero counts. after_fulfillment instead
requires the unforgeable settled value, obtains the retained actual application
with exact owner/variables checks, then joins the same private selection helper.
That helper checks the same Cx/source world and actual callee, uses original numeric
families/keys, genuine EarlyBinder callee Args and ordinary upstream deep resolution.
No replacement Args/ParamEnv or general-variable-to-numeric relabeling was added.

TrueError and residual Ambiguity retain distinct typed upstream outcomes.
collect_remaining_errors drains actual residual work into Ambiguity records with
its real stalled_on/Maybe information; that is not success. Original obligations
remain separately retained even after the pending context drains. Fatal overflow
retains the original work and live FulfillmentContext with its pending/stalled
state plus the exact overflow obligation and earlier errors. If the later drain
returns fatal overflow, already returned errors are preserved in overflow.errors.
No residual/error/overflow branch can construct SettledRootApplication, convert
fatal overflow to ambiguity, or expose a mutable retry/registration accessor.
The read-only retained_fulfillment getter exposes only a shared context reference.

Snapshot evidence is the existing pinned-style num_open_snapshots equality
assertion on application access and on FulfillmentContext, together with exact
owner/variables pointer identity. It is a depth invariant, not a new source/epoch
or unique snapshot-generation certificate. Source currentness, packet ownership,
root/source-query admission and outer inference lifetime/rollback boundaries
remain the future admitted runner's responsibility. The private fulfillment
method can retain ordinary upstream inference progress on unresolved outcomes;
this review does not claim failure rollback or runtime coercion adjustments.

## Architecture and cost review

1. Existing pinned FulfillmentContext performs all semantics; the new method only
   registers actual relation outputs and classifies its typed terminal result.
   No assertion/expression evaluator or independent trait solver was introduced.
2. Source clauses and original call relations retain ordinary declaration and
   source identities; no literal/scalar-only generated path was added.
3. Numeric demand scans real instantiated TypeIR occurrences, independent of
   trait/method/assertion names, primitive widths or candidate matching. It is
   direct metadata incidence, not a numeric answer or full constraint component.
4. No generated-source exclusion or identity bypass was added. Existing source
   plans and later admission remain authoritative for original/generated origins.
5. No pattern machinery, cold executable-arm checking or VM execution was added.
   Equality/coercion results cannot certify runtime conversion/call commit.
6. Preparation retains encoded relation/source plans. Fulfillment clones cheap IR
   obligation handles once; it does not reparse source or reconstruct a body.
   Existing whole-Q80 encoding before incidence filtering remains an explicit lazy
   activation hold for unrelated unsupported bounds; this change does not fix it.
7. No namespace scan/global comparison or new invalidation policy appears.
   Owner/variables/depth checks are constant-time. Source demand selection visits
   original variables and, if any stay open, actual encoded clauses and their
   TypeIR structure. This metadata work belongs to preparation, not dispatch.
8. No immutable BodyIndex clone or whole-file executable node search was added.
   Original obligations, caller environment, callee Args and clauses are retained
   as existing typed handles; mutable inference belongs to the same actual owner.

Cold costs: O(number of original obligations) storage/registration, plus the
existing pinned fulfillment engine's actual progress work and rustc recursion
limit. No custom loop/scheduler or resource cap was introduced. Failure can retain
both original obligations and pending/error handles, preserving provenance.
Numeric selection costs depend on original variables and actual visited clause
TypeIR; no semantic closure/candidate work is performed locally. There is no
active installed-execution/hot-path call site yet and no measured performance
claim. The future admitted runner must supply genuine source/query closure,
packet-currentness and full ordinary source-constraint component integration
before any numeric fallback or VM commit is considered.

No tests/Cargo/runtime commands ran. The unchanged repr128 witness, exact-once
argument effects, suspension/resume, source invalidation and full macro/edition
matrix remain unexecuted for this changed component. This review authorizes no
root activation and closes no M020/runtime/JIT/architecture gate.

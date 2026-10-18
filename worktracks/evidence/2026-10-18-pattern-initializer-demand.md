# Reached constant patterns demand the ordinary VM initializer

Root, LOCAL-044, WT04/T032/T033, E-R5-005/E-R5-007. Actual unchanged issue-17718
V15 crosses const function-pointer calls and static/atomic assertions, then fails
at original C1 => {} line45: engine::run_pattern_names selects Constant then calls
constant_item_value/constant_item_data, entering an obsolete pure runtime evaluator
with incompatible empty-fragment source authority. This is architectural drift;
repair the route rather than relax source seals or expand pure constant syntax.

Shared joins: retain PatternSelection::Constant definition identity in PatternTarget;
ordinary non-consuming Search reports a typed initializer demand only if it reaches
an unresolved constant (including a range endpoint). Do not eagerly initialize all
names or cold alternatives. Frame separates the same search result from transactional
publication. The VM operation dispatcher retains its exact operation index, leaves
scrutinee/guard/binding/cleanup owners unchanged and reports a typed initializer exit
before completing the operation. RunMachine enters existing demand_initializer /
CallResolver::initializer / ordinary body/GenericEnv/return/run-long storage. On retry,
the retained pattern name takes its actual VM value; the same Search compares it.
No second pattern engine, initializer interpreter, fake source body or macro case.

Per-use initializer storage gains actual definition identity alongside PC, so two
reached constants at one pattern PC cannot receive each other's tickets. Existing
value reads use that same key. Scalar/shared intrinsic values retain const_cells;
nonCopy templates/structural constant equality remain held, never guessed. Remove
obsolete constant_item_value and constant_item_data production helpers when no
callers remain; preparation-time pure constant queries remain intentionally separate.

Cold costs: existing name classification once per retained tree/PC. Hydrate only
newly VM-initialized target slots without source queries; clone Arc targets only
when a cached target becomes ready. Hot ready pattern search uses existing retained
tree and data, no parser/body/namespace walk. A cold initializer suspension restarts
only the non-consuming search at the exact operation; no earlier recipe op, scrutinee,
guard or initializer is re-executed. Re-search cost O(reached pattern prefix) per new
initializer demand, not a plan rebuild or ordinary dispatch cost. No speed claim.

Required evidence: unchanged original root+real auxiliary, existing whole-pattern
transactions/range/false-guard retries and initializer controls; LRR cold alternative,
resumption/side-effect/exact-once controls outside imported originals. All-eight
review must verify one matcher/evaluator (1-2), no macro/inference/member case (3-4),
cold alternative and transactional ownership (5), no execution planning (6), no
new namespace scan/invalidation (7), Arc retained tree/body (8). Pure source-constant
operation legacy routes, generic associated initializer cache identity, nonCopy
run-once templates and structural constant equality are remaining independent holds;
no full pattern/E007/interpreter/JIT feature closure is implied.

## Implementation review snapshot

- `crates/lrr/src/engine/run.rs`: `d931fc46695b9f721441ed22e609f5596e479268e0a40680f74623a93413f622`
- `crates/lrr/src/engine/run_capture.rs`: `31badbd705f878983630a1301159c0b933b7a3796dcfe3f0c78895e56df81c9d`
- `crates/lrr/src/engine/run_initializers.rs`: `1200c6c6cb4c034931d78d6e3722a6b8dd271ed44b55dd0b4fda84dca41567f6`
- `crates/lrr/src/engine/run_pattern_names.rs`: `66ca50adb90c915018b5f776ef67663426becce31afdd31e724b6d3b2f15d6db`
- `crates/lrr/src/engine/run_pattern_tests.rs`: `465a7ccbcffed7a23d0b4ca92e22f20d58d7ea02cba65b461230e41f6c1c0397`
- `crates/lrr/src/engine/run_source_constants.rs`: `4de745231d1459759bd4fb831b8744a308edb77e9374712ed8bdd813cf80f24c`
- `crates/lrr/src/engine/run_value_items.rs`: `421e708f91eb91777deaf472deff9c2078d50d7d6fad36ff3ac8c2da86fa99b7`
- `crates/lrr/src/vm/frame_pattern_engine.rs`: `ab12515de52d83aa4489afe6af8f7c86edcda2640c69b99310a8e93edd740927`
- `crates/lrr/src/vm/pattern_match.rs`: `48c664e0b8cb344355668e4297fe99b459700639d1b551214eebd91592a6a061`
- `crates/lrr/src/vm/step.rs`: `caf6a1748c8f37f30a8d6259be7258ba92a96753051fbe6b8eadeae978f3db5d`
- `crates/lrr/src/cli/initializer_tests.rs`: `84ffac60e9bd071dc8602fb7cf2f520858e051a38a3ea849d830168640002bc4`

Same Search::run is called once per test attempt; typed SearchFailure propagates a reached constant without publishing a result. Exact Test operation suspends before charging/counters/commit, retaining rerun_index. Ordinary initializer demand survives NeedInput until actual entry. Re-search after return retains the same retry skip and no prior operation or guard is replayed. Cached scalar hydration clones target metadata only when a pending slot becomes ready. Non-scalar targets remain explicit unsupported with their original owning tickets retained; full structural equality and cleanup/template integration remain held. Constant helper removal leaves preparation-time pure queries and historical source-constant operation models separate. No build/runtime result is claimed.

Source review explicitly keeps per-run O(names) cached-target hydration as runtime data work (no name resolution or planning). No scaling/performance acceptance claimed. Main/source cache and generic activation identities retain the listed existing holds. Ordinary unit and structural/ref constant equality remain unsupported; no source spellings or macro names select an implementation. Two supplemental route controls and the strengthened old constant control are unexecuted until V16 terminals.

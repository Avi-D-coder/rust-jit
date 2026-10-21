# M020 fact-factory and reached-root validation plan

LOCAL-049 / WT05-02. This is an unexecuted plan. Work directly in the macOS
checkout with pinned Rust 1.99.0 and one Cargo owner; use no copied sources,
private targets or worktrees. Begin only after the lead freezes source inputs,
generator/vendor inputs and HEAD, and explicitly transfers Cargo ownership.
Keep those inputs unchanged until every selected process is terminal and closing
guards have been recorded. A later repair needs a new run directory and guards.

## Minimal commands

`RUN_DIR` and `REPORT_DIR` below are placeholders for fresh, concrete paths
recorded before execution. The report directory must not already exist.
Launch each long Cargo or runtime command asynchronously and collect its actual
terminal before starting its dependent phase. Preserve exact expanded argv,
session/terminal IDs, exit status and output. Continue independent source review
without modifying frozen inputs while commands run.

```sh
/opt/homebrew/bin/python3.12 tools/validation/source_manifest.py RUN_DIR before
cargo +1.99.0 test --offline --locked -p lrr --lib --no-run --message-format=json > RUN_DIR/lib-build.jsonl 2> RUN_DIR/lib-build.log
cargo +1.99.0 build --offline --locked -p lrr --bin lrr > RUN_DIR/cli-build.log 2>&1
cargo +1.99.0 build --offline --locked -p xtask --example lrr_source_digests > RUN_DIR/digester-build.log 2>&1
/opt/homebrew/bin/python3.12 tools/validation/source_manifest.py RUN_DIR after-build --cargo-messages RUN_DIR/lib-build.jsonl --cli-binary target/debug/lrr
shasum -a 256 target/debug/examples/lrr_source_digests
```

Any failed build ends its dependent stages. Still record closing source/HEAD
guards, actual fresh/stale binary status and terminal outcome before returning
ownership. A successful build proves only that registered components compile.

For any newly assigned source component control, first use the exact existing
test selector from the actual Cargo-produced test executable's `--list` output.
Record it in a maintained one-control selection, then run:

```sh
/opt/homebrew/bin/python3.12 tools/validation/run_controls.py RUN_DIR TRACKED_SELECTION
```

No selector is invented by this plan. No synthetic evaluate/WellFormed root or
unrelated broad control set is selected to make a partially ready provider run.

When the actual common pre-fallback source/inference/root/response consumer is
installed, the runtime selection is the unchanged original alone:

```sh
LRR_EMBEDDED_FACADE=1 /opt/homebrew/bin/python3.12 tools/rustc_checkout_runtime.py --lrr target/debug/lrr --source-digester target/debug/examples/lrr_source_digests --report-dir REPORT_DIR --timeout 240 --case tests/ui/enum-discriminant/repr128.rs > RUN_DIR/runtime.log 2>&1
/opt/homebrew/bin/python3.12 tools/validation/source_manifest.py RUN_DIR after-runtime
shasum -a 256 target/debug/examples/lrr_source_digests
```

The maintained harness preserves owning Rust 2015, actual run-pass directives
and imported source identity. This command does not compile a native reference.
Test/CLI guards come from the actual Cargo artifact and closing manifests;
independently compare the digester's complete before/after hashes. Host control
success, interpreter runtime-contract pass, full-fixture acceptance and any
separate compiler evidence must have distinct fields. The historical V523 result
is executor LRR-E-OP-TYPE/CLI exit2 with no runtime-check acceptance; it is a
baseline, never an expected-success adapter. A component-only factory checkpoint
does not require rerunning an unchanged inactive runtime path merely for logs.

## Actual root closure required for repr128

1. Capture the reached `discr` call's real argument/result relations before
   `bind_literal_fallback`, retaining source/family identity and once-evaluated
   argument cells. The projected declared requirement is a callee obligation,
   not a caller assumption.
2. Materialize actual source facts with query-specific availability. Exact
   DiscriminantKind/associated Discriminant/PartialEq IDs, owning generic layouts,
   real enum repr and complete actual candidate buckets must reach the same IR
   context. Missing facts cannot become empty/false answers. Unrelated fields
   may remain Unprepared when the actual callback closure excludes them.
3. Admit that specific source-bound root set and actual caller ParamEnv in one
   inference owner. Authenticate the nongeneric missing-env/entered-env caller
   branch. Derive callback closure from the actual query domains; a globally
   ready flag or fake WF goal cannot authorize the real roots.
4. Use pinned normalization, candidate selection, inference and fulfillment.
   Retain certainty, external constraints, pending/overflow and original source
   variables separately. In particular, normalize the actual discriminant
   projection and infer U from genuine PartialEq candidate inputs before the
   ordinary numeric fallback fixes an unconstrained literal.
5. Export and consume only the same owner's real response. Revalidate source
   receipts, exact active call/environment handles, attempt identity, argument
   snapshots and current result expectation before commit. NeedInput/resume and
   repeated call sites must preserve exact-once effects and disallow stale reuse.
6. Feed inferred literal bindings into the existing runtime operation, with no
   primitive retagging, assertion-name rule, second projection solver or body
   inference. The ordinary numeric operator and its cleanup owner remain shared.

Root callback closure, response application, installed consumption and stale
activation/commit refusal need their own source review and controls. A passing
repr128 runtime is one unchanged runtime contract, not the full origin/edition,
cold-branch, suspension, invalidation, dictionary or milestone matrix. Those
remaining entries stay explicitly unexecuted until actually exercised.

## Durable evidence before cleanup

Before removing disposable outputs, write a compact capsule under
`worktracks/evidence/` containing expanded commands, actual terminals, exact
source/HEAD/input count, actual test/CLI/digester hashes, selected control body
count and measured observations, original fixture hashes/directives/edition,
runtime report status/fault/checks and guard outcomes. Preserve failures and
unexecuted dependent stages. Record the separately reviewed source fingerprint.
Only report measurements parsed from actual output, never intended counts.

Keep raw outputs until their owners are terminal and the capsule is recorded.
Then remove only reproducible temporary projects/programs and private build
directories after checking active owners and unique inputs. This run creates no
private target or full snapshot. If raw outputs later disappear, describe the
retained capsule and unavailable evidence directly; do not invent raw hashes,
capture identities or historical command details.

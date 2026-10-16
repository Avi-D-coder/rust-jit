# Shared reached scalar pattern tests — bounded architecture record

Owner: `/root/common_pattern_engine`, under coordinator `LOCAL-024`, October 15,
2026. Baseline `8e4e1c23c8b5800c54fc3262f8179d0b1b552a4c`, with inherited dirty
coordinator changes preserved. T030/T050/T051, WT03-01/03. This is an intermediate
source component, not completion of a task, runtime fixture or E-R5-005.

The violated invariant is duplicated non-consuming pattern-test semantics.
`match x { -7..=5 => (), _ => () }` and `if let -7..=5 = x {}` currently use
different signed-literal decoders, scalar-family checks and comparison lowerers.
The unchanged upstream witnesses are `tests/ui/binding/match-range.rs`,
`tests/ui/binding/match-range-infer.rs` and
`tests/ui/match/match-on-negative-integer-ranges.rs`, already imported with their
existing provenance. Originals and their directives remain unchanged.

Ordinary source enters `ParsedFile::initial_thunk` with its admitted `BodyHandle`,
`BodyIndex`, source/profile and indexed PC. Match selection is owned by
`literal_match::prepare_inner`; if-let, while-let and condition-chain phase
selection enter `condition_ranges::prepare`. Both use `draft` and the ordinary
`ir::validate::verify` route, then ordinary `Literal`, `NumericHint`,
`NumericBinary` and `Discard` VM operations. `ScopeEnter`/`ScopeLeave`, source
slot lifetime, guard continuation and final result transfer remain the existing
owners' responsibility. Hygienic generated syntax must join this same indexed
route through coordinator-admitted retained syntax; this slice grants no new
source, namespace or macro authority.

Extend those actual entry points with a shared `syntax/thunks/pattern_tests`
module that retains actual literal/range AST handles, performs reached endpoint
admission/decoding and lowers one comparison. Remove the old
`scalar_match_ranges` production module and the duplicated decoder/comparison
code in `literal_match` and `condition_ranges`. Coordinator removes the obsolete
bool-condition decoder in `initial_let_condition_thunk` once the common route
selects bool literals. Source constraints remain distinct from pattern tests:
match's source hint and condition source hint retain their existing authority.

No source parser, evaluator, macro-name rule, query framework or runtime opcode
is added. Source preparation is pure and bounded to existing AST nodes. Endpoint
family inspection uses token kinds/suffixes without magnitude decoding; the
upper endpoint is decoded only at its reached continuation. One comparison
allocates a bounded three-operation sequence, with condition slot-hint forwarding
using the existing ordinary `NumericHint`. Installed execution consumes only
prepared opcodes. Existing match-wide plan reconstruction remains explicitly held
architecture debt, not a repaired cost.

Required closure evidence: existing unchanged original-main controls, retained
source/route controls, exact comparison/cleanup recipes, cold upper bounds and
cold result calls, plus source review against all eight recurring failure modes.
The sole validator owns all Cargo/test execution on frozen source. No Rust test
has been run by this worker. General structural pending-binding transactions,
false-guard alternative retry, let-else, generated scope/source admission and
macro-specific assertion routes remain held prerequisites/repairs. A negative
unsupported control is never a successful Rust runtime contract.

## Frozen implementation handoff

The shared entry points are `ScalarTestPlan::from_source`, `first`, `upper` and
`ScalarComparison::operations` in `syntax/pattern_tests.rs`.
`literal_match::prepare_inner` and `condition_ranges::prepare` both select them;
there is one reached endpoint-family check, signed decoder and scalar comparison
lowerer. Source-type constraints use `pattern_tests::source_type` without
decoding magnitudes or executable result bodies. Conditions preserve their
ordinary runtime `NumericHint` forwarding from the actual scrutinee slot; match
supplies its already prepared source hint. Neither parameter grants source
identity or selects semantics by macro name.

`scalar_match_ranges.rs` is deleted. The old bool-only condition decoder in
`thunks.rs` is deleted by the coordinator and bool now uses the shared route.
Open ranges previously admitted by conditions now use the same plan for match;
the old match-specific unsupported-unbounded control was replaced with positive
shared-route/runtime controls. Historical negative receipts remain historical
refusal evidence and never become accepted Rust contracts.

The old blanket `generated_origin` rejection in `condition_ranges::prepare` and
provenance-only early `None` in `literal_match::context` are removed. Intrinsic
literal tokens acquire no namespace authority. Names still use
`local_binding`/`parameter_at` → `binding_name_matches` →
`same_local_name_context`; parameter sources still require their existing
guarded `DefinitionParameter` admission. `primitive_cast_target::resolve` still
requires actual generated Type-name proof and refuses without it. This removes
no source/currentness/namespace check. The valid Rust cross-context definition
scope witness remains explicitly unsupported; the new authority control forbids
capturing its same-spelled caller as a fallback.

Coordinator dependency: `ParsedFile::binding_let_statement` in
`syntax/node_lookup.rs`, plus its `node_lookup_tests.rs` original-source
correspondence control. Both local-source consumers now use that exact indexed
Pattern interval, existing body-content/entry validation and immediate owning
LetStmt attachment. `conditions::condition_plan` and
`require_boolean_match_guard` now use `entered_node_syntax` rather than scanning
all descendants for an already indexed owner. These remove four rediscovery
scans. They do not remove body hashing, semantic source demand, enclosing lexical
search, complete condition-map construction or complete match-plan construction.

Frozen worker source SHA-256:

| File under `crates/lrr/src` | SHA-256 |
| --- | --- |
| `syntax/pattern_tests.rs` | `b19c22e6a048c14912dda7b953f438de6f0a675416e7b558f9bd2e2990f6b34f` |
| `syntax/pattern_tests_tests.rs` | `578c87f506c413ca3d1905c74909ed9c3121007051eb868c814f05caf8d41654` |
| `syntax/literal_match.rs` | `d08e1233aac18eb916d97299e06bdda8c129e1946bd6d71a8d010a172204e80b` |
| `syntax/condition_ranges.rs` | `7753efd1fef2141ad70a8593a9a6c8a3b402007612b87a1a9315547442945477` |
| `syntax/conditions.rs` | `a5fce4077fe156a6b1cdac92e373f81519c0c4533487cfb5afa85d8dca849999` |
| `syntax/scalar_match_ranges_tests.rs` | `14b574ae3989e627785e61bdf3af11095c7a6623d28a5da90ff52e7de6039b47` |
| `service/common_pattern_route_tests.rs` | `19a1de7daf36492e32e28761b53146fc171ae73de7a0897a0fcb2e874193d0db` |

Coordinator glue at review time: `syntax/thunks.rs`
`4e56b4c62d2232f27e521596df504475dab0e69e09e13dc518625be2bbd27994`;
`service/mod.rs`
`f11c792c786222fddd6bdcb1eb3705ebf0d76097409fc5ff0a0a7fc9a5498c3d`;
`syntax/node_lookup.rs`
`ed72037a490f73fc5c230c76b63420ab1f4812574867ec1ea673dba95d424bea`;
`syntax/node_lookup_tests.rs`
`ba0d3eef1d6a864378a649e13ca175ee8e16cec1fe7c2f9589a9bc713332573f`.
The validator must freeze the combined source and record its actual complete
manifest; these worker hashes alone do not identify all concurrent root/reference
changes or establish a build result.

## Separate source-aware architecture review

This review inspects the frozen sources above and the named adjacent authority
routes. It is source evidence only: no compilation, runtime, milestone or speed
claim is made. The recurring failure-mode numbers below are distinct from the
audit's `MA` IDs.

| Recurring failure mode | Inspected symbols, result and remaining hold |
| --- | --- |
| 1. Separate assertion/expression evaluator | `ScalarComparison::operations` is reached from ordinary `literal_match::prepare_inner` and `condition_ranges::prepare`, then normal `draft`/`ir::validate::verify`. It adds no evaluator. `runtime_macro_scalars` still dispatches `controls.match_expression`/`if_expression`; MA001 and the assertion portion of MA002 remain held. |
| 2. Literal/break-only macro output | New tests use genuine `RetainedSyntaxInput` publication and normal `BodyStructure`/`Thunk` query keys for arbitrary user/wrapper macros. The shared test module has no macro-output branch. Existing special generated-expression/control executors outside this slice remain held under MA004/MA005; standard-macro common-route acceptance is unexecuted. |
| 3. Context-specific methods/numeric inference | Shared `source_type`, `floating_context`, `family` and `decode` use actual token kinds/suffixes, with no macro identity or method selection. Ordinary `context`/`source_hint` retain their existing partial constraint services. `runtime_macro_numeric_context::operand_name` still constructs synthetic parsing input and assertion-only numeric inference remains; MA003/MA006 are not repaired here. |
| 4. Generated-source rejection | Removed both old blanket checks described above. `primitive_cast_target::resolve` remains the actual Type authority boundary, and hygienic local/parameter checks remain required. Cross-context definition-site lookup and generated Type proofs are still explicit gaps, not spelling-based fallbacks or claimed compatibility. |
| 5. Duplicate patterns/eager cold arms | One scalar test planner/decoder/lowerer replaces three copies; `first` never decodes the upper endpoint, and test preparation never visits result expressions. `vm/pattern::PatternContext` remains binding modes only; `plain_binding_header`, scalar `binding_draft`, identifier/wildcard conditions, general transactions, overlapping false-guard retry and let-else remain held. `context` still has its preexisting unconstrained nested-match result collector: this is not complete lazy source-type inference. |
| 6. Repeated parsing/planning | New test stage reads retained AST handles and emits bounded comparisons, with no parser or query call. Installed VM operations execute prepared literals/comparisons only. `literal_match::plan` still builds/sorts all indexed source rows and reconstructs arms per phase; `condition_plan` still constructs a complete source-location map per demand. Retained memoized plans remain required. |
| 7. Full-world namespace work | New module has no namespace lookup or census. Existing `ScalarPatternBindingAdmission::require` still rechecks `lexical_namespace_stamps`; other common call/Type services retain existing census debt. No installed-guard scaling or unrelated-invalidation repair is claimed. |
| 8. Immutable copies/whole-file lookup | New plan retains at most two shallow AST handles, not cloned `BodyIndex` structures. Four owner/declaration rediscovery scans were replaced with the existing direct-range service while preserving its source checks. Full plan/index walks above remain. No release performance measurement was performed. |

## Witnesses and validation plan

Existing unchanged upstream bank and manifest remain untouched. The existing
`fixtures/upstream/rustc-checkout/manifest.json` pins rustc
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, MIT OR Apache-2.0, including:

| Original | SHA-256 |
| --- | --- |
| `tests/ui/binding/match-range.rs` | `2df51e522ebeda27b15a88a6d7275766b464f76f31af1b0ac402bd7be7e4d8ca` |
| `tests/ui/binding/match-range-infer.rs` | `92c12147663967ee443767d126338a204c1bca80d601e31f1b2ea51a67a1b1d2` |
| `tests/ui/match/match-on-negative-integer-ranges.rs` | `d55223d431c9d0840087bcbfe2f04c369654919e262a3d5b3cb9ca9132f1b7b3` |

Three new `syntax::thunks::pattern_tests::tests` bodies inspect the actual
ordinary thunk → shared lowerer route with a test-only lowering counter, compare
the unchanged eleven-range bank to external conditional adapters, cover
bool/byte/open ranges, enforce non-consuming/scratch-cleanup operations and reject
a wrong owning body. Four new `service::common_pattern_route_tests` bodies use
actual source discovery/publication, verify ordinary indexed nodes and query
routes, assert zero assertion-evaluator opcodes, exact comparison/call counts,
and cold-call non-preparation. They exercise handwritten/user/wrapper output,
single-alternative false guards, if-let/while-let, condition chains, cold upper
sources, bool success/mismatch and cold literal-to-call replacement. One of the
four is an explicitly negative source-authority control, not a valid-Rust
runtime-contract pass.

All seven new bodies and existing fixtures remain **uncompiled/unexecuted** for
this source. Existing seven `scalar_match_ranges_tests`, six `condition_ranges`
tests, prior retained source/epoch controls and the full library require rerun.
The retained macro matrix has no standard-macro shared-route result; binding
transactions, cleanup of owning aggregate moves/borrows, panic and suspension
remain unexecuted or unsupported here. Existing original-main tests that use the
legacy assertion route do not prove E-R5-005 common execution.

Commands for the sole validator's frozen macOS Rust 1.99.0 source/target, not run
by this worker:

```sh
cargo test -p lrr --lib syntax::thunks::pattern_tests::tests::
cargo test -p lrr --lib service::common_pattern_route_tests::
cargo test -p lrr --lib syntax::thunks::literal_match::scalar_match_ranges_tests::
cargo test -p lrr --lib syntax::thunks::condition_ranges::tests::
cargo test -p lrr --lib syntax::node_lookup::tests::
cargo test -p lrr --lib
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- gate 03
```

Actual worker terminals: `rustfmt --check --edition 2024 --config
skip_children=true` on the seven frozen worker Rust files, exit 0;
`git diff --check`, exit 0. Neither command compiles or executes Rust. No Cargo,
Rust test, source checkpoint commit/push, ledger completion or integration was
performed by this worker. Any later source change needs a new fingerprint and
affected review/validation rerun; these conclusions stay attached to this source.

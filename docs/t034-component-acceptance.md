# T034 component acceptance and mandatory integration

Section57 distinguishes already implemented component acceptance from final
revision4 acceptance when later ledger tasks provide required integration. T034's
named minimal harness contract is covered by real original-source guest runs:

| Requirement | Acceptance evidence |
|---|---|
| Selected case succeeds despite an unselected broken body | `selected_test_runs_without_preparing_an_unselected_broken_body`, CLI counterpart and cold-body query/install observations |
| Exact, substring, ignored, listing and no-match behavior | `test_listing_exact_substring_and_no_match_follow_the_cli_contract` and catalog selection controls |
| Fault cannot satisfy should_panic | `expected_guest_panic_accepts_numeric_panic_but_never_an_unsupported_fault`, source macro, binding type and expansion-fault controls |
| Fresh guest heap for every selected case | `test_cfg_and_multiple_selected_cases_execute_with_fresh_guest_heaps`, `repeated_selected_test_execution_creates_independent_machines_and_finishes_each_run` |
| Assertions and panic return actual guest outcomes | VM opcode/admission controls, source-resolved identity/alias controls, literal, source-call, local and parameter dispatcher controls |
| Fixture manifest/harness format | Strict Section28 schema parser/checker controls, actual on-disk original source preparation and terminal outcome collection |

Gate03's component check requires every registered T030–T034 host acceptance test
to appear as passed in the fresh workspace output. A missing, ignored or failed
selected test rejects the gate. Gate03's report explicitly retains pending broker
transcripts, generated discovery/operands/origins, actual native tiers and the full
fixture matrix. `full_acceptance_passed=false` and `release_complete=false` remain
explicit. This check does not pass or skip any pending full-runtime fixture.

The task ledger retains named pending acceptance for the on-disk contract, general
runtime macro arguments, MX014/MX015/MX017, MX027/MX028 and MX033. Their component
controls do not count as full-LRR evidence. General/comparison operands, formatting,
macro-generated catalogs and diagnostics, output/drop capture, ownership-sensitive
operands, tier equivalence and complete Section28 grading require the listed later
T/M tasks. Re-run full guest assertions after their integrations and M043; final
release evidence must satisfy the existing ledger admission checks. The frontend,
query framework, VM, JIT, authority and sandbox choices remain unchanged.

The current original-source CLI uses the provisional development schemas. No
Linux or full profile/release support is claimed. Component completion permits
continuing the fixed order with H001–H003, then M013–M016; it does not remove any
original or revision4 requirement.

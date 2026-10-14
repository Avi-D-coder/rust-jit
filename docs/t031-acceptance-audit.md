# T031 control-flow component acceptance audit

Current H002 statement-scope integration adds ordinary scope entry/retirement.
The normative main now executes 12 operations and uses exactly 23 fuel; 22 fuel
faults. Eight-operation and 19/18-fuel figures below describe the historical
checkpoint. See [the current boundary integration](h002-statement-scopes.md).


Revision 4 Section 27 T031 requires selected branch fetch, labels, break values,
continue cleanup, backedge polling and deterministic fuel. The original-source
component exercises those paths through the actual lazy frontend, verified LRIR,
physical frame and dispatcher. It does not establish later generated-source,
reference/user Drop, multi-frame service or JIT contracts. E-57-001 preserves
those requirements as pending integrations in the fixed Section 57 order.

| Requirement | Observed component evidence |
| --- | --- |
| Selected if/else-if only | `lazy_if_selects_only_the_reached_arm_and_propagates_primitive_context`; cold arm node IDs absent from actual thunk query traces |
| Lazy boolean RHS | `short_circuit_boolean_ops_skip_cold_rhs_and_execute_selected_rhs_once`; selected RHS completes once; skipped undefined RHS has zero thunk events |
| Hot while and fresh iteration storage | `while_reenters_fresh_iteration_storage_and_reuses_installed_thunks`; direct/block conditions terminate, each installed PC queried once |
| Explicit loop/backedge boundary | `backedge_admission_requires_loop_entry_and_preserves_exit_identity`; observed backedge and poll counts agree |
| Break values and continue cleanup | `loop_break_values_and_continue_retire_selected_scopes`; source loops return correct reached values and skip unreachable successors |
| Lexical labels | Nested loop/while/labeled-block tests, lexical shadowing and rejected missing/non-loop targets; break result admission preserves crossed scopes on rejection |
| Deterministic fuel | Exact 19-unit source success versus 18-unit failure, exact loop exhaustion, terminal replay rejection; unit counts distinguish opcode, exit and cleanup elements |
| Interruption | Pre-cancel/deadline produce no thunk execution; another thread cancels a live guest loop after observed charges; scalar cleanup cancellation preserves owned cells |
| Admission before charging | Numeric, state, local, ownership, exit and literal controls reject with exact diagnostics before failed-op charge or output allocation |
| Cleanup ownership | Actual owned strings are retired one cell per fuel unit; exhaustion leaves the next owned cell and active scope intact; repeated reentry regression detects stale locations/objects and payload leaks |

Full generated control/phase revalidation remains under MX014/MX029 after M014,
M016/M017 and M043. Source member/type applicability is M030. Calls and shared
multi-frame cancellation/status need T032/T033. Reference leases and general
user Drop/partial-object cleanup need T041–T045. Bound operation identity,
resumable activation chunks and JIT/supervisor agreement remain mandatory in the
later dispatcher/cleanup/tiering integrations. In particular, the present scalar
scope scan does not prove Section 16.7's bounded activation/chunk contract.
Component completion does not make these obligations passed or release-complete.

The final checkpoint must contain actual Gate00/01/02 results, every selected
T031 test observed passed, source digests and strict Clippy/format/architecture/
manifest evidence on macOS Rust 1.99.0. Linux acceptance remains deferred.

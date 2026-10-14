# T030 block/local component acceptance audit

Current H002 statement-scope integration adds ordinary scope entry/retirement.
The normative main now executes 12 operations and uses exactly 23 fuel; 22 fuel
faults. Eight-operation and 19/18-fuel figures below describe the historical
checkpoint. See [the current boundary integration](h002-statement-scopes.md).


Revision 4 Section27 T030 and Sections11.2/13 define this component. Section57
places it before T031's source loops and T041's references/places. Thus the
loop/reference and general place acceptance requirements cannot be fully proved
here without changing the fixed order. The existing
[E-57-001 component/integration rule](errata/e57-component-integration-order.md)
preserves those requirements as pending; component completion is not release
completion. The final checkpoint records the actual checks and task status.

| Requirement | Component evidence and remaining integration |
| --- | --- |
| Block templates | Entered Entry/Commit recipes activate scope, schedule only reached statements, transfer tail/unit results, explicitly leave nested scopes and continue. The block result cell belongs to its enclosing expression scope. Source nested/unit/typed blocks and exact scope counters pass. |
| let and runtime storage contracts | Initialized bindings and DeclareLocal use stable pattern-node identities and physical cells. LocalHint reads an already executed local's declared/bound contract. Uninit locals initialize once, initialized immutable writes fault, writable replacement preserves established descriptors, and unresolved numeric aliases retain shared binding. Source positive values and exact negative fault codes pass. |
| RHS-before-LHS assignment | Entry schedules the RHS; destination commitment follows Operand(1). Bare local identity supplies context without executing the LHS. Overflowing RHS panics before missing/immutable/general destination rejection; a query trace observes RHS before destination commitment and zero LHS thunk fetch. Nested RHS assignment produces the correct changed local value. Full effectful field/index/deref destination evaluation remains T041/M030 integration. |
| Shadow initialization | Existing and nested source controls read the preceding binding during the new binding's initializer. Inner names do not escape completed scope; outer bindings retain their values. Ordinary/hygienic/symbolic routing integration remains mandatory. |
| Source invalidity after return | Explicit return inside nested blocks completes without preparing later invalid inner/outer statements. Cold syntax still indexes structurally. |
| Reincarnation and ownership | The frame helper is driven through 1,000 real enter/write/bind/leave cycles. Each binding location is fresh, old cell epochs and object handles reject, slots/bindings clear, and 4 MiB cumulative string payload completes within a 2 MiB budget by releasing owned bytes. This is actual frame/storage evidence, not a source-loop or reference-lease proof. T031/T041/T042 must supply those final assertions. |
| Fail-closed and deterministic IR | Modified/labeled blocks retain explicit unsupported boundaries. New local opcodes have stable tags, typed slot/binding admission, effect classifications and counters. Invalid identities, missing slots and output/source aliasing reject. The normative main retains eight operations and zero counts in new columns. |

Current T030 component inventory has eight checks in the task ledger and fixture
registry. The full workspace also exercises storage ownership, type/context,
laziness and numeric regressions. Gate02 freshly observes required fixture passes;
its final report, strict Clippy, formatting, architecture and manifest audits must
pass before recording component completion. These checks do not imply Gate03.

Pending final evidence is retained for T030-ASSIGNMENT-RHS-FIRST (full effectful
place behavior), T030-SCOPE-REINCARNATION (source-loop/reference integration),
MX014 and MX029 (generated source and release integration). Named prerequisites
include T031/T032, T041–T045, M014/M016/M017/M030/M043. Revision4 TR033 must prove
source-dependent compound assignment order through M030: primitive versus generic
order is selected before either operand and cannot be guessed from a runtime tag.
The current adapter guards compound assignment rather than silently supplying the
wrong order. No full compound/source-view support is claimed.

General destructor scheduling, temporary/borrow extension, all pattern forms,
aggregate destinations, generated/hygienic lookup, exact source views, consolidated
quotas/fuel/cancellation, service contracts, JIT agreement and release/security/
performance evidence remain the specified later work. Linux requires actual
acceptance and remains deferred. No selected live runtime unit is compiled by
this component; queries remain effect-free owned-recipe construction.

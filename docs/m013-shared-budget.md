# M013: shared run expansion accounting

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: discovery domains and procedural run ownership can share one request,
unit-graph and epoch-bound output ledger. Identical immutable outputs charge once;
distinct outputs share the token/wire/invocation cap. A foreign owner or procedural
epoch refuses, and overlapping callers cannot commit competing stale totals.

SharedExpansionBudget owns stable output descriptors under a safe mutex. Planning,
source admission and accounting commit form one serialized transition. Count/size
checks precede index growth, and typed errors preserve counters on refusal. Limits
freeze on first discovery poll or macro execution. Attaching a new domain to an
already active ledger is allowed; changing the active ledger's limits is refused.
Counters return typed outcomes rather than hiding a poisoned ledger.

Discovery keeps its existing explicit integration paths and exact record checks.
Compiler observations/produced operands use the shared ledger; source admission
holds the accounting transition through actual integration. UnitGraph gains a
separate explicit stable encoding for ledger ownership; the existing manifest
schema and its encoding are unchanged.

MacroRun creates or accepts a matching shared ledger. After the existing separately
authorized matched-server invocation and input verification, it checks the validated
output before coordinator adoption. Quota refusal follows the existing run-error
cancellation path and cannot yield an input certificate or invoke the helper again.
Pure output-cache facts may still exist; they do not authorize a closed run.

Four controls cover cross-domain reuse/exhaustion, foreign request/graph/epoch and
late configuration, concurrent competing charges, and an actual prepared proc tool
in the matched external server. Native evidence proves one invocation, refused
output adoption, unchanged database revision, no retry, and failed seal verification.
The actual native record also refuses charging into a foreign epoch ledger.

Complete impl/test catalogue plumbing (M014/M015), origins (M016), all required run
limits/deadlines/cancellation/supervision and broader producer-cycle integration
remain mandatory. The ledger is usable by both owners, but full CLI/procedural
source integration and all named release fixture reporting remain unfinished.
Body-position user expansion, comparison assertions, type/JIT/security/release and
Linux acceptance remain pending. M013 is not marked complete from these controls.

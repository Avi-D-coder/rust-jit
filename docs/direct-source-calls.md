# T032 direct source-call execution — work in progress

Original-source calls with a simple unqualified function name and no written
arguments and admitted primitive/unit return contracts now execute through the LRIR interpreter. The entered thunk owns a
CallSeal name/site recipe and a matching Call exit with result slot and resume PC.
Structural admission checks the call node, bounded name and matching exit. Names
bound by lexical locals or local items do not fall through to a module function;
those callable forms remain explicitly unsupported.

The dispatcher uses the paired immutable source catalogue and value namespace to
select the exact source callable. It prepares the reached zero-argument contracts
and selected body structure before executing CallSeal; pure query suspension
therefore preserves the caller PC and spends no call opcode charge. An uncalled
signature/body and unused generic bounds remain unforced. CallSeal owns a sealed
argument batch, even when empty. The Call exit suspends the caller; the dispatcher
consumes the batch once, publishes a callee on the shared run meter, and retains
the selected source identity, contracts, result slot and caller continuation.

Reached primitive returns check their descriptor, clean the callee's scopes and
transfer the ticket once into the caller result slot before resuming its saved PC.
Return preflight and reached root result mismatches report
LRR-E-CALL-RETURN-TYPE. Missing direct identities report LRR-E-NAME-MISSING. Existing
exact counter assertions gained the zero CallSeal column; assertions for the
previously unsupported missing-call and return boundaries now require these
specific diagnostics.

Callee arithmetic panic unwinds the physical frame stack through the existing
scalar cleanup machine, with counters accumulated across frames. Recursion uses
one shared fuel meter and a checked 32 MiB retained call/frame metadata budget.
The metadata estimate includes both owned body indexes, frame slot/binding/scope
arrays, signature storage and fixed overhead. A source return identity that is an alias is explicitly deferred before body entry
until symbolic body hints are integrated; primitive-looking alias names never
inherit a primitive descriptor from their spelling. Explicit unit return syntax
is admitted. The guest arena also retains its
run handle/resource limits. Runtime faults stop immediately without guest cleanup.

Six controls exercise nested calls and exact seal counts with a cold invalid
signature; missing/local-item/local-value identity guards; explicit/implicit/unit
return mismatches and recursive fuel exhaustion; multiframe panic cleanup; and
recursive call metadata exhaustion before a large fuel budget; and actual CLI
subprocess execution of nested source calls with observed VM seal/cleanup counts. Failed compile/test
attempts are preserved rather than reported as passed.

T032 remains in progress. Written/excess arguments, parameter frame initialization,
full generic and return-only inference, source-aware alias/owner/member/function-
value/native classification, complete SourceView/ParamEnv/RuntimeSubst/dictionary
IDs and context-bearing CallPacket, coercions, generated-source/macro suspension,
user destructors and release integration remain mandatory. Existing raw primitive
body hints retain their earlier component scope; this checkpoint does not establish
complete symbolic return-type semantics. Validation is macOS aarch64 on Rust
1.99.0. Linux support remains deferred until actual port acceptance.

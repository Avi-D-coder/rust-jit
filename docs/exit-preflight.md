# T031 exit preflight — work in progress

Recipe exits now undergo immutable admission before their fuel unit is charged.
Branch validates the actual bool; return admits its source owner/state and reached
primitive descriptor without moving it. Break/continue validate active selected
targets and loop eligibility. Break additionally admits its incoming value and
result destination before retiring any crossed scope, and rejects storage that
would itself be crossed by cleanup. Backedge validates readiness to enter its
indexed loop scope. Execution reuses return/control admission.

Invalid exits do not consume their unit or increment completed exits. Earlier
committed ordinary opcodes remain visible. Explicit Fault exits remain charged
instructions; unimplemented call/effect boundaries fail admission until their
ordered tasks supply execution. Structural PC/ancestor checks remain the verifier's
responsibility, separate from these runtime checks.

Three added controls prove wrong branch/return leaves values live with no exit
charge, inactive continue preserves the outer scope, and a mismatched break result
preserves source, crossed scopes and handle/byte totals. Existing selected control
flow, loop storage, numeric panic and interruption tests remain regressions.

Literal admission now has [a preflight path](literal-preflight.md).
T031 remains in progress. Cleanup/aggregate element charging still needs
implementation. Scalar control admission is not a proof of user Drop,
reference leases, staged cleanup activation, call/effect/native packets, full
multiframe/service/supervisor/JIT integration or final release acceptance. Those
cross-ledger requirements remain mandatory. Linux acceptance remains deferred.

Integrated as scoped T044 components from collected V13; fresh MAIN milestone remains pending. Historical draft text follows; it is not full task, rustc fixture or release acceptance. See artifacts/macos-first/t044-call-continuation-integration-v13/integration.json.

V12 was not executed: independent preflight caught its missing fixture registration. V13 adds that registry entry and updates task metadata, with production and all tests identical to V12.

# Private T044 exact call-return continuation V13 — unvalidated

V11 passed its repaired original unit-pair control and ledger check, then failed the unchanged `runtime_macro_predicate_call_keeps_verified_seal_and_typed_nonboolean_result_fault` integration test. The selected assert macro predicate call uses a verified temporary destination; the old preflight incorrectly required that destination to equal the enclosing AST node's final value slot. The actual diagnostic was LRR-E-INTERNAL-IR instead of the required LRR-E-OWNERSHIP. The failure and 22 unexecuted checks remain recorded.

V13 retains the exact resume PC and result slot at the verified Call exit, refuses a second pending continuation, and checks that pair before destination allocation or returned-owner consumption. Successful resumption clears the pair once. Canonical PC, suspended-frame, source/type and initialization checks remain. The retained pair has no heap allocation and its full inline size is included in checked frame metadata accounting. No macro special case or arbitrary destination admission is added.

The original rustc return-nil fixture supplies a prepared caller frontier for the additional authority regression: a different canonical resume and a different result slot each refuse without fuel, counters, handle or owner changes; the legitimate original continuation still completes afterward. This is an LRR authority adaptation outside the unchanged licensed upstream tree. The unchanged macro integration regression must pass as well.

Production changed, so V13 requires a fresh full library run (473 expected bodies), format, strict Clippy, ledger, all 17 selected call/scope/source integration binaries, importer/import integrations and architecture audit. No library passes are carried from V9/V11. All32 T044 controls and 167 upstream cases/287 original files remain registered. Full rustc compiletest passes remain0; T044 remains in_progress, release_complete=false. V10 replacement remains separate and excluded.

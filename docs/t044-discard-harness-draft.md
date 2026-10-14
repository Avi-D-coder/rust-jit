Integrated as scoped T044 components from collected V13; fresh MAIN milestone remains pending. Historical draft text follows; it is not full task, rustc fixture or release acceptance. See artifacts/macos-first/t044-call-continuation-integration-v13/integration.json.

# Private T044 bounded scope-scan harness correction V11 — unvalidated

V9 passed 471 of472 library bodies, with one failure in the unit-pair statement control. The control assumed its original ScopeLeave metadata scan completed in one call. V11 preserves the unchanged source and accepts only same-PC Dispatch at the exact ScopeLeave opcode, with the pair already fully retired, one Discard charge, unchanged field-cleanup count, and no next-source execution. A bounded64-resumption loop must reach the original Next exit; other outcomes fail.

Production and every other test are identical to V9. The failed V9 run and terminal/release receipts remain preserved. V11 requires the exact repaired control plus the unexecuted integration/ledger/importer/architecture suffix. The 471 unchanged library passes may be carried only after byte/production identity verification; no single all-green472-library run is claimed. All31 T044 controls and167 upstream cases/287 original files remain registered; full rustc compiletest passes0. T044 remainsin_progress and release_complete=false. The mutable local replacement V10 is separate and excluded.


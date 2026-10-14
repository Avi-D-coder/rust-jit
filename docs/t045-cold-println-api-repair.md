# T045 V10 pinned API repair

V9 formatting passed; strict Clippy exited101 during compilation on Rust1.99.0 macOSARM64. No library/target/subprocess tests ran. Its source stayed fixed. The external gate receipt and raw diagnostics are preserved.

This unvalidated successor repairs three mechanical mismatches: profile identity comes from PreparedSourceView.selection().outline.parse, source-relative original-node checks use the actual matching BodyHandle range rather than a BodyKey field, and generated runtime-argument inventory handles std Println with zero non-message operands, matching the pinned source argument parser. Every existing explicit enum arm and syntax/identity/authority check remains. No fixture, expected message, size budget, deadline or task requirement changes.

Expected493 library/15T045 controls and177cases/297 unchanged originals; no V10 tests yet. Full fixture/compiletest/userDrop/output/worker-isolation/release/Linux acceptance is incomplete. Guest JIT remains deferred.

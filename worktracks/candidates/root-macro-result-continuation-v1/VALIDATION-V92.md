# Scoped evidence on the combined V92 candidate

Pinned macOS Rust1.99.0 source `4d40beec1122538cb3a87176b1de676b82021936131dd2c0c960c36c14848b92` passed production library Clippy (`ec1c6b/0`) and library test compilation (`c9f7db/0`). The entire selected place/panic/terminal-call bank passed **36/36** (`ab3461/0`), including the exact original-PC bounds message, u64::MAX non-truncation, empty-array identity checks and terminal-call continuation control. The unchanged borrowed-str main also passed across2015/2018/2021/2024.

V92 aggregate was108passed/2failed; its CRLF preparation failures remain preserved. Scoped passes do not convert that aggregate into a full milestone pass. Subsequent helper repairs have separate evidence. All source,435extras,507raworiginals and owner guards matched. No canonical source promotion, configured compiletest full-fixture pass, task/JIT/Linux completion follows. See [V92 receipt](../../evidence/2026-10-15-v92-string-numeric-bounds.json).

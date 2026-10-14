# T031 stock division, float exponent and u32 decrement observations

Collected scoped execution; original files are now integrated. The original rustc sources
`tests/ui/numbers-arithmetic/div-mod.rs`, `float2.rs` and `u32-decr.rs` retain
their exact bytes and run-pass directives from
ae0fe511b15ab760d7e963b13389d1bc2ab42b18. Both upstream license texts and COPYRIGHT
remain unchanged. Their original assertions cover division/remainder with typed
isize operands, f32/f64 scientific notation and comparisons, and u32 decrement.
The three programs contain 20 original assertions in total.

The exact previously built T034 scalar-assertion v3 CLI executed all three
programs and all20 original assertions. Separate explicitly authorized native
references in original edition2015 and adapted edition2024 also passed.
Source and binary hash/size/mtime stayed unchanged. The observer built no LRR
code and did not rerun already observed cases. MAIN retains127 cases/221 original
files after these six additional originals were integrated. Production was
unchanged by this import; fresh combined MAIN validation is pending.

[Collected observation](../artifacts/macos-first/t031-stock-div-float-u32-v1-scoped-observations/index.json)
and [six-file integration](../artifacts/macos-first/t031-stock-six-additional-originals-integration-v1/integration.json)
retain actual terminals, source hashes, assertion counts and collector-only
failure evidence. The scope notes were updated after copying to reflect the
collected observations; copied rustc files and license texts remain unchanged.

Unchanged source under LRR edition 2024 is an explicit edition adaptation. It
does not establish full compiletest, guest JIT, release or Linux acceptance.
The existing unresolved isize cast-source refusal and aggregate-cleanup failure
remain recorded; these additional cases do not replace or hide them.

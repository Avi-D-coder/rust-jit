# Source-bound UTF-8 string literal IR candidate

Status: source-only, uncompiled and unexecuted. This nine-file T022/T024 candidate follows the coherent [48-file byte prerequisite](../root-interpreter-bytes-v1/README.md). Its touched source leaves do not overlap the later byte assertion/admission changes. Claim LOCAL-024 retains ownership. No task is marked complete and no new upstream runtime pass is claimed.

String literals now carry immutable UTF-8 data decoded by the pinned Rust parser library, alongside their exact original spelling and body span. Decoding normalizes actual CRLF only in its scratch content; escaped CR/LF, Unicode and NUL values remain meaningful data. Fatal escapes fail, nonfatal continuation warnings do not change the decoded value, and checked source/metadata budgets bound retained data. These receipts do not authorize guest storage, reference coercion or library member execution.

Ordinary and macro scalar literal lowering use the common source decoder. The verifier requires a matching string payload rather than accepting a string with absent decoded data. Stable encoding adds decoded-string tag 7 without renumbering existing tags; the recipe schema and fingerprint domain advance from 13 to 14. Both existing wire migration controls are updated. String VM allocation, `str` source context, equality, methods and `include_str!` integration remain outstanding; the current VM still reports unsupported string execution.

Three pending component test bodies consume unchanged upstream sources: CRLF spelling/value/offset preservation, Unicode escapes, embedded NULs, continuation warnings, bare-CR lexical rejection, and source-payload refusal in real initializer recipes. The diagnostic and check-pass originals provide component evidence only. Whole main execution, compiler diagnostics and full fixture acceptance remain separate and unproven.

Two original run-pass cases were additively imported into the canonical fixture inventory using the normal importer (`90426a/0`), with both existing rustc MIT/Apache license texts retained:

- `tests/ui/parser/unicode-escape-sequences.rs`
- `tests/ui/str/nul-char-equivalence.rs`

Canonical inventory now contains 314 cases and 498 original files. These are import counts, not passes. The active validation source still contains its own frozen 312 cases/496 originals. Before compiling this candidate in that source, add the two cases with `tools/import_rustc_tests.py`; preserve its aggregate manifest and any preceding byte imports. The candidate patch deliberately excludes fixture manifest replacement and original duplication.

Static receipt `c9b4ea/0` checked all nine before hashes, replayed the patch to exact after hashes, and verified five source witnesses against the pinned checkout. Pinned Rust 1.99 formatting completed in `37689c/0`. Patch SHA-256: `e8e284a876f6c05146b231ea37fd2bc656042f9abd2547d01100518cbe7779fb`. No Rust build or runtime command ran. The active gate's source and target were untouched.

Hold this candidate behind the active milestone and already assigned byte validation queue. Future validation requires strict compilation, the three new token/IR controls, affected literal/wire scopes, and eventual string runtime integration with unchanged whole-program witnesses. Source preparation alone does not satisfy their runtime contracts or the guest JIT prerequisite.

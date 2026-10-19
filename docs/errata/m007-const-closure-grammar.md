# M007 ordinary const-closure grammar repair

The pinned rust-analyzer parser 0.0.356/commit
bcf0514497034e1156f3cc2bb72b99cc9634a949 already admits `const` closures in
expression position. Its `grammar::items::opt_item` nevertheless consumes
`const move || ...` as an item modifier at block statement/tail position, before
ordinary expression parsing. The unchanged rustc regression
`tests/ui/traits/const-traits/const-closure-parse-not-item.rs` and real core
`iter/traits/double_ended.rs` expose this mismatch.

Return the existing marker to ordinary expression parsing when `const` is
followed by a closure qualifier or pipe. Existing const declarations, const
functions/traits/impls and const blocks keep their ordinary routes. This is a
mechanical grammar repair, not a change to LRR's execution architecture or a
static const checker. No diagnostic is suppressed, source text rewritten or
runtime closure implementation added.

Both pinned parser package instances carry the same repair: the published
ra_ap_parser package used by ordinary source and the existing vendored parser
used by the macro adapter. The package version and compiler commit remain
fixed. `vendor/UPSTREAM.toml` records exact preimage, patch and postimage hashes;
`tools/bootstrap_vendor.py` and `tools/audit_parser_vendor.py` independently
reconstruct the patched files from retained upstream archives. ParseKey's domain
and the source expansion engine version change to separate old preparation.

The exact check-pass original supplies structural component evidence only.
`const-closure-fn-ptr-const-item.rs` is an independent runtime witness whose
interpreter result must be collected; importing or parsing it is not acceptance.
Library source closure, namespace receipts, full macro-context equivalence and
the interpreter prerequisite for guest JIT remain open.

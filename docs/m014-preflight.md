# M014 preflight: nested declaration producers

This is preparation for the next Section 57 task after M013 component acceptance.
No M014 implementation or acceptance is claimed here.

Task and invariant: M014 scans active declaration skeletons in functions, blocks,
closures, async bodies and anonymous consts without preparing executable bodies or
type-checking expressions. Nested impls can affect reached selection even when
their containing functions never execute. Unknown macros remain producer demands;
cfg-disabled subtrees and inert stringified tokens must not contribute declarations.

Two unchanged official rustc cases are copied under the original licenses:

- `tests/ui/lint/non-local-defs/local.rs` contains inherent/trait impls inside
  functions, local types and anonymous const blocks. Its 2021 edition directive
  stays intact. Its cold `todo!` method bodies cannot be executable-prepared merely
  because their impl headers are scanned.
- `tests/ui/lint/non-local-defs/weird-exprs.rs` contains impls inside array-length
  expressions, an enum discriminant and function signature const expressions.
  Its warning expectations and stderr remain intact. Structural discovery is
  distinct from reproducing rustc lint output.

The next acceptance tests should consume these original bytes through the pinned
rust-analyzer parser, retain source/edition/cfg identities and report structural
scan cost separately from prepared executable counters. LRR-specific controls
must cover cold ordinary type errors, disabled subtrees, closure/async containers,
and unknown macro frontiers. Audited intrinsic grammar-slot summaries must scan
Rust expression arguments recursively where required, while `stringify!` token
arguments remain inert. A spelling match does not establish intrinsic identity.

Use the existing source/header identities, deterministic discovery worklist,
owned token/origin boundary and authorized expansion coordinator. Index actual
impl headers; do not fabricate a completed ImplCatalogSeal from a syntax-only
inventory. Expression/block expansion admission and complete graph/facade coverage
must precede sealing. Symbolic impl/associated header interpretation and reached
method selection integrate with the later term/goal/dictionary tasks.

MX029/MX030 remain pending. The two imported cases are unexecuted and are not
added to the passing component or full-fixture counts. Linux remains a later port.

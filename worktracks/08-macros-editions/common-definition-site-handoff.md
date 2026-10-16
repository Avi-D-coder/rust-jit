# Common definition-site source identity prerequisite

Coordinator `/root`, LOCAL-024, T024/T063/M014/M017, WT08-02; baseline
53ab9d3f97eb4e963b65f24d39c0df1ea9e0669e. Source-only successor to the separately
frozen pattern/reference packet. No task, architecture or runtime acceptance.

The current minimal witness is `let x=7; macro_rules! read{()=>{x}}; let x=9;
read!()`: actual local comparison classes refuse the missing definition-site
scope join rather than capture 9. The existing pinned-compatible
frontend/local_scopes.rs resolver already removes marks only across the matching
macro declaration rib. It must be reused, not replaced by a lexical algorithm.
Unchanged macro-nested_stmt_macros.rs and macro-stmt-2.rs supply source attachment
and repeated/wrapped origin witnesses; existing retained_lexical_tests.rs records
the unresolved valid-Rust execution. Originals remain unchanged.

First implement the missing shared source identity: SourceSyntaxAssembly carries
actual original/generated macro declaration occurrences through exact adapter
SyntaxEdit receipts and records the actual issued expansion -> source macro
identity/owner/edition. Original pairing happens before any expansion, using the
captured physical name span inside its source scan declaration. Surviving anchors
follow mapped actual ranges. Inserted declarations pair with the exact admitted
output scan by fragment position and actual capture order within that one edit;
identical origin keys do not identify occurrences. Validate exact capture/body
fingerprints against actual final macro nodes and encode every range/identity
in PreparedSourceSyntax identity. Retain count/byte limits and checked allocation.

This metadata is structural preparation, not an executable-name result, Type
proof, new namespace, seal or VM authority. Existing source/epoch/seal/record
admission must remain required. The subsequent ordinary entered-body index will
join these occurrences and actual token contexts to existing LocalScopes once,
under the existing pure query/cache. Only then can ordinary local/parameter
lookup use definition-site ribs. Never resolve by macro spelling or adjust a
context using an invented algorithm. Procedural contexts without a matching
source declaration are an explicit separate prerequisite.

No runtime evaluator, parser, macro-specific semantics, cold executable-body
preparation, dispatch plan reconstruction, namespace census or immutable BodyIndex
copy is introduced. This cold assembly work is bounded by declaration/output
counts; it is not a claim that existing repeated source preparation or global
namespace costs have been repaired. Eight-mode source review and actual frozen
validator checks are required; broader source-name/CLI/standard macro integration
and all eight legacy macro shortcuts remain open. This first identity component
alone does not pass the minimal execution witness.

## Separate source review and freeze

Reviewed the exact four leaves in `artifacts/macos-first/common-definition-site-v1/SOURCE.json`.

1. No assertion/expression evaluator: assembly metadata only.
2. No output executor: actual adapter tree edits and declaration captures retained.
3. No method or numeric inference; declaration owner/edition comes from opaque issued request and exact scan.
4. Same occurrence mechanism handles original and generated declarations; original cfg exclusions mint no identity. Procedural contexts and actual resolver join remain explicit prerequisites.
5. No executable pattern/body admission or cold-arm inspection; only required declaration/macro syntax is captured.
6. Existing cold materialization is reused before/after each attachment. Macro-body capture hashing adds bounded cold work; no installed-operation or per-dispatch planning. Full source preparation costs remain held.
7. No namespace census or invalidation change; actual source/record/epoch/seal admission stays external and required.
8. No BodyIndex copy or new whole-file node search for an entered phase. Original physical-name joins use a bounded ordered index; inserted occurrence pairing uses the actual one-edit preorder, not origin-key uniqueness.

Only rustfmt and targeted whitespace checking have run; no Cargo, native, runtime, library or milestone terminal exists for this successor. Prior frozen validation snapshots remain unchanged. The separately numbered MA audit remains fully open.

- `crates/lrr/src/expand/source_declarative/assembly.rs`: `d378901a7d9c98396710f9c7ab6d123f5c227a81a21010c212895410d70c79e8`
- `crates/lrr/src/expand/source_declarative/assembly_tests.rs`: `8e0191f8da4555322c43c76aa4d77155b55acfcad5207791befe9e0c11557e85`
- `crates/lrr/src/expand/source_declarative/macro_anchors.rs`: `8400840854df3e36bf958c396d517aec4bb964d3c8c3b30b087ed5f166cc5f4c`
- `crates/lrr/src/expand/source_declarative/syntax_receipt.rs`: `d3c32e10fd2f82a8a312a2bc79b81dfd61ef431bd812fdc913aeceb66e5b8053`

V2 supersedes the **unexecuted** v1 packet: original physical name lookup now uses the existing validated strictly sorted TokenForest span table directly (binary lookup), removing the duplicate full span-map allocation. The inspected adapter contract is token_wire/model.rs new/validate strict span-key ordering. Other source/authority conclusions remain unchanged. Exact packet `artifacts/macos-first/common-definition-site-v2/SOURCE.json`, SHA256 `14eb5de66b6e3aaefb0575f394a79c72d89daa97310c44e543aacaa2e5973fcd`. All Rust validation remains unexecuted.
- `crates/lrr/src/expand/source_declarative/assembly.rs`: `d378901a7d9c98396710f9c7ab6d123f5c227a81a21010c212895410d70c79e8`
- `crates/lrr/src/expand/source_declarative/assembly_tests.rs`: `8e0191f8da4555322c43c76aa4d77155b55acfcad5207791befe9e0c11557e85`
- `crates/lrr/src/expand/source_declarative/macro_anchors.rs`: `08c6f8be99091d51e4eaa998a80708adbb5e325e71c65aaa9fa1f16f3d5eb64d`
- `crates/lrr/src/expand/source_declarative/syntax_receipt.rs`: `d3c32e10fd2f82a8a312a2bc79b81dfd61ef431bd812fdc913aeceb66e5b8053`

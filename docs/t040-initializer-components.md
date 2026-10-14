# T040 initializer source components — integrated component

This component extends the current constructor metadata and source-binding path to exact original constant/static initializer expressions. It is integrated after scoped validation and has passed its fresh MAIN macOS milestone:2160 host/doc tests,0 failures/ignored, all45 registered T040 controls once, and the fresh default-binary interpreter check. Full T040, constant evaluation, general tuple/array execution, user Drop, guest JIT, release and Linux acceptance remain pending. Full stock fixture passes remain zero.

Task invariant: source and body hashes, pinned AST boundary, source declaration/header, cfg, profile and original hygiene must agree before initializer metadata or a nominal constructor witness can be captured. Original source frontiers remain typed demands; initializer callers do not manufacture a function signature. Existing scope encodings0–8 are retained; Initializer is appended9.

The common entered-body helper accepts a non-block expression only at the direct initializer of an original constant/static, including a single-token literal. It rejects changed contents and forged interior field expressions. An initializer lifetime scope encloses a root expression and any distinct control scope. Existing function block roots retain their structure. This metadata is not a constant evaluator; general initializer frame entry, continuations and execution remain pending.

Entered macro capture visits a non-block root itself, retaining one actual outer macro invocation and keeping nested macro operands opaque. ConditionPlan can capture an original root if without evaluating unresolved names or selecting a branch. ConstructorPlan retains written field order and checked declaration-order storage mapping.

Source-aware nominal binding obtains generic parameters from the actual constant/static declaration. Functions still demand their real FunctionSignature; associated owner and field/type/import/producer requirements remain deferred. Capturing generic metadata uses checked sizes and fallible reservations within the existing32MiB source-recipe ceiling, with a deterministic duplicate-name index preserving lifetime and type/const namespaces and source ordinals. Stable signature/token encodings are unchanged.

Effect classification:

| Helper/path | Effects |
| --- | --- |
| ParsedFile::entered_body_root | Pure pinned AST boundary and content verification; bounded temporary owned hash input; no query, compiler or guest effect |
| ParsedFile::initializer_parameters | Pure checked declaration/header and owned generic syntax capture; no bound solving or evaluation |
| PreparedSourceView::initializer_parameters | Read from an already retained immutable source lease; no new filesystem read or publication |
| generic_tokens / capture_generics | Pure metadata allocation with checked storage sizes and fallible reservations; no name resolution or compiler execution |
| bind_record_constructor initializer branch | Reads existing database inputs and source lease; preserves normal source/schema queries and typed deferrals; no FunctionSignature or Constant query for an initializer caller |
| interpreter harness default target | Developer-side binary path selection respects CARGO_TARGET_DIR; execution provenance still requires explicit current source/binary evidence |

Unchanged pinned rustc sources are retained with original directives, snapshots and licenses:

- consts/const-struct.rs: original y field order b,c,a versus declaration a,b,c; pure map1,2,0; original std::cmp/Debug frontier remains deferred.
- consts/const-as-fn.rs: original FOO=0 token-sized root; compile-fail FOO() semantics are not selected.
- macros/nested-macro-expansion.rs: original static FOO concat invocation; nested operand tokens remain opaque; no expansion/static evaluation/main pass.
- consts/control-flow/basics.rs: original ABS_DIFF conditional and unresolved X < Y clause; no constant evaluator or whole run-pass.
- const-generics/defaults/self-referential.rs: actual duplicate const M identity and token quota refusal; no complete compiler stderr parity.
- consts/issue-19244.rs: original import-free STRUCT nominal witness; nested constants, arrays and original main are unexecuted.

All eight new registered controls and sixteen affected regressions passed on the frozen v4 candidate, with strict Clippy and fresh architecture. The actual outer driver26398 ended1 because its final binary-mtime predicate compared against the later interpreter start. That failure is preserved; a separate read-only audit verifies actual fresh build interval/SHA and selected native/interpreter command0. This is scoped passing evidence with an external validator failure, not a successful single driver. The later integrated MAIN driver13409 and collector57148 both exited0, with all nine checks passing on3630 unchanged source/include inputs through collection. Its fresh binary build interval and SHA match the default harness report. The harness target-path fix is mechanically motivated by the verified pre-integration MAIN target/debug/lrr default; no stale binary observation establishes current interpreter acceptance.

[Collected scoped evidence](../artifacts/macos-first/t040-initializer-expression-controls-v4-scoped-evidence/independent-freshness-audit.json). [Integration and preimages](../artifacts/macos-first/t040-initializer-source-components-integration.json).

[Integrated macOS milestone](../artifacts/macos-first/t040-initializer-source-components-main-milestone-v1/independent-collection.json). [Terminal collection receipt](../artifacts/macos-first/t040-initializer-source-components-main-milestone-v1/collection-terminal.json). The original tested integration receipt is retained inside that evidence directory; this document's post-collection update changes only reporting.

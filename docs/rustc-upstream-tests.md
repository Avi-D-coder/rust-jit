# Official rustc test reuse assessment

Current MAIN retains111 unchanged official rustc cases/204 original files at ae0fe511b15ab760d7e963b13389d1bc2ab42b18. Full stock acceptance remains0. The latest macOS milestone failed the bounded-stack recursion check despite all77 registered construction/reference controls passing; selected AST/expression/macro checks are partial fixture evidence. Separate unvalidated cumulative drafts retain115/209. The earlier whole-source empty-derive CLI/native-reference result is scoped interpreter evidence, not complete compiletest or JIT acceptance. [Current receipt](../artifacts/macos-first/t041-implementation-progress-v1.json).

## Historical reuse assessments

Current checkpoint: M016 include and procedural diagnostic components are integrated after scoped private acceptance. The preceding collected macOS milestone passed all nine checks, 2103 actual host/doc tests and23 M016 controls. The new coherent integration requires a fresh milestone. Inventory77 cases/157 retained original files; full stock fixture passes0. M016 remains in progress; full MX033/MX034, guest JIT, release and Linux remain pending. See [integration evidence](../artifacts/macos-first/m016-cold-compiler-integration.json). Earlier checkpoints below are historical.

Current inventory64 unchanged cases/131 original files, including both license texts and COPYRIGHT. Literal include helpers can be retained explicitly with --support-file without becoming test cases; same-stem .fixed expectations are retained alongside .stdout/.stderr. The include-single-expr and expr_2021_cargo_fix_edition closures are source-only imports with unexecuted full language expectations. Three actual stock-based importer integrity controls passed privately; integrated milestone pending. See [the closure witness and correction](errata/m015-stock-fixture-closure.md). Earlier inventory entries below are historical.

The user's official checkout at `rustc/rust` is now the primary source for language
fixtures as features are implemented. A second selection at commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` initially contained eight derive cases with helpers,
snapshots, both licenses and the original copyright notice. See
[the licensed checkout import](../fixtures/upstream/rustc-checkout/README.md) and
[the generated derive component scope](m013-generated-derive-facts.md).
The import now retains seventeen cases and 37 unchanged files. The new local/const-expression
impl cases now pass declaration-syntax/inventory assertions for M014, retaining
ten and six actual impl headers in their original 2021 source facets. Full native
lint and LRR fixture acceptance remain pending. See
[the scanner scope](m014-declaration-scan.md). M013 component acceptance
is complete under E-57-001; full MX027/MX028 remain pending. See
[the acceptance audit](m013-acceptance-audit.md). The current
derive-to-attribute work uses the second item of `attribute-after-derive.rs`,
adapted outside the licensed folder to SDK exports which assert configured derive
input and raw following-attribute member tokens. Replacement, prior generated
macro binding and stale-source controls pass; full stock fixture execution and
stdout/diagnostic compatibility remain pending. See
[the continuation scope](m013-source-following-attribute.md). Original-source identity and chained
forwarding now retain exact helpers. Passing isolation controls confirm that
sibling and function-like child helpers remain unresolved and cannot publish. See
[the original helper inheritance scope](m013-original-helper-inheritance.md).
Seven cases have passing component assertions through a pinned identity-macro
expansion, including namespace-selected annotations and replay. Their full native
and LRR fixture expectations remain pending. See also
[the resolution scope](m013-generated-derive-resolution.md) and
[the cfg/materialization scope](m013-generated-derive-input.md). The derive-order
component also uses the production standard compiler annotation facet;
[the annotation scope](m013-compiler-annotations.md) records its exact limits.
The copied derive-order case also exercises ordered choices in a controlled pure
export inventory; [the export scope](m013-derive-exports.md) distinguishes that
component from the actual authorized SDK suspension regression.
The [helper adoption scope](m013-helper-adoption.md) adds LRR-specific coordinator
and suspension regressions using the prepared SDK fixture. It does not add an
upstream case pass; language expectations continue to come from the copied tests.
The [helper scope checks](m013-helper-scope.md) reuse derive-helper-configured and
derive-helper-shadowing-2 without changing their source; only component scope is
accepted so far. Full upstream native/compiletest/LRR expectations remain pending.
The [authorized invocation control](m013-derive-invocation.md) runs the prepared
SDK derives through MacroRun and retains their output pending append integration.
It is supplementary authority/ordering evidence, not an imported full fixture pass.
The [append/publication control](m013-derive-append.md) integrates the prepared SDK
derive outputs and publishes their discovery proof through the owning run. It
supplements the upstream cases; their full acceptance remains pending.
Use `tools/import_rustc_tests.py` to copy additional cases unchanged, record exact
commit/hashes, and preserve license terms. Keep harness adaptations outside the
copied source directory. LRR-specific authority/suspension/cache/seal regressions
supplement the upstream language tests. The earlier pinned-compiler selection
below remains separate evidence from a different revision.

The initial import is pinned to Rust 1.99.0 compiler commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`. It contains six official UI cases,
one procedural helper, its diagnostic snapshot, and both upstream licenses.
Every imported file was verified against the Git blob identity from the official
repository and has a SHA-256 in
[the manifest](../fixtures/upstream/rustc/manifest.json).
Sources remain unchanged. This is a selected import, not the complete rustc suite.
The fetched recursive repository inventory was truncated; it is not evidence of
an exhaustive suite assessment.

Rustc uses [compiletest](https://rustc-dev-guide.rust-lang.org/tests/compiletest.html).
Its [UI modes](https://rustc-dev-guide.rust-lang.org/tests/ui.html) distinguish
checking, building, running and compiler rejection. Reuse must retain the
[directives](https://rustc-dev-guide.rust-lang.org/tests/directives.html), revisions,
edition, target restrictions, auxiliary crates and expected output files.
The imported cases use the upstream default edition 2015; converting them to 2024
would be a separate, explicitly labelled variant. LRR must preserve definition
edition when consuming their macros.

| Official case | Reusable behavior | LRR integration still required |
| --- | --- | --- |
| `for-loop-macro-rules-hygiene` | Caller identifiers survive definition-local bindings | Runtime expression macros, vectors/indexing/assertions; M013/M030/M043 |
| `macro-tt-followed-by-seq` | Token-tree repetition and recursive expansions | Runtime macros, generic enums, Debug derive and formatting; M013/M028/M033/M043 |
| `macro-tt-matchers` | A token-tree becomes a generic type constructor | Type aliases and managed Box; M013/M017/M032/M043 |
| `user-defined-macro-rules` | A user macro named macro_rules generates an item | Item discovery; M013/M043; upstream check-pass is not runtime equivalence |
| `associated-types-basic` | Associated projection resolves to the impl's type | Demand-driven projection and runtime assertion; M023/M043 |
| `gen-macro-rules-hygiene` | Generated locals, labels, items and dollar-crate retain distinct origins | Separate LRR semantic assertions and origin/property work; M016/M043/M044 |

The final case expects rustc compilation errors. Its helper and stderr are retained
for understanding the witness. It was not sent to the native expected-pass harness.
LRR's relaxed profile does not require identical rustc diagnostics or unconditional
compile-time rejection; reached lookup and hygienic identity need explicit assertions.
Compiler-internal, LLVM/MIR, rustdoc and borrow-check diagnostic suites likewise
need classification before selection rather than blanket compatibility claims.

Explicit native reference preparation on macOS ARM64 observed three run-pass cases,
one build-pass case and one check-pass case succeed with the exact pinned compiler.
The commands, output, compiler identity and manifest snapshot are in
[the native report](../artifacts/macos-first/rustc-upstream-td7qgtsx/report.json).
Reproduce with `python3 tools/rustc_upstream_oracles.py`.
This is test-only Rust compilation; no LRR query or selected live runtime unit
invokes the compiler. Procedural preparation and execution retain their separate
authorities when the procedural fixture is integrated.

**No imported case has passed full LRR execution acceptance.** These native observations do not complete
T100, M043, any release gate or Linux acceptance. Integration follows Section 57;
the manifest records pending cases without counting them as skipped successes.

The M014 local macro checkpoint adds the unchanged run-pass
`tests/ui/macros/macro-nested_stmt_macros.rs`. Two component assertions resolve
and expand the original cold invocation with preserved spans under LRR2024; the
original upstream directive is retained. This raises the exercised component
assertions to 40. Full compiletest and LRR fixture acceptance remain zero. See
[the scope](m014-local-macros.md) and
[checkpoint](../artifacts/macos-first/m014-local-macros-checkpoint.json).

The generated declaration checkpoint exercises the unchanged
`macro-nested_stmt_macros.rs` output as a statement fragment with a retained
nested producer and actual output-origin rows. The newly imported unchanged
`macro-named-default.rs` exercises root expansion and associated-demand suspension.
Both component harnesses use LRR2024. Seventeen cases and 37 imported file hashes
retain 42 component assertions, with no full fixture acceptance. See
[the scope](m014-generated-declarations.md) and
[checkpoint](../artifacts/macos-first/m014-generated-declarations-checkpoint.json).

The generated-capture checkpoint exercises the unchanged statement fixture's
actual `foo!()` path/argument group, original spans and marked contexts. This
raises component assertions to 43 across seventeen imported cases and 37 verified
files. Full native/LRR fixture acceptance remains pending. See
[the scope](m014-generated-captures.md) and
[checkpoint](../artifacts/macos-first/m014-argument-declarations-checkpoint.json).

The M014 nested-intrinsic integration adds unchanged
`tests/ui/macros/assert-eq-macro-success.rs` from the recorded checkout commit.
There are now 18 imported cases and 38 verified original files. Its component
argument-syntax control exercises every assertion operand and formatting tail
without executing string conversion, allocation, derive traits or formatting.
That control passes, but the full run-pass fixture remains unexecuted by the
compiletest contract. The next milestone is running; the frozen 1748 checkpoint
still records 17 cases, 37 files and 47 component assertions for its earlier scope.

The accepted M014 nested-argument checkpoint now records 1756 host/doc passes,
18 imported cases, 38 unchanged verified files and 48 upstream component
assertions. Full compiletest and full LRR fixture acceptance remain zero.
The generated-argument milestone is running and has no new accepted count.

The generated intrinsic checkpoint passed 1770 host/doc tests and all 86 M014 controls, retaining 18 imported cases, 38 unchanged verified files and 48 upstream component assertions. Full compiletest and full LRR fixture acceptance remain zero.

The generated intrinsic checkpoint passed 1779 host/doc tests and all 95 M014 controls, retaining 19 imported cases, 39 unchanged verified files and 49 upstream component assertions. Full compiletest and full LRR fixture acceptance remain zero.

The procedural capture checkpoint passed 1792 host/doc tests and all 108 M014 controls, retaining 20 imported cases, 43 unchanged verified files and 50 upstream component assertions. Full compiletest and full LRR fixture acceptance remain zero.

The procedural capture checkpoint passed 1798 host/doc tests and all 114 M014 controls, retaining 20 imported cases, 43 unchanged verified files and 50 upstream component assertions. Full compiletest and full LRR fixture acceptance remain zero.

M015 adds unchanged ambiguous-builtin-attrs-test.rs, its builtin-attrs.rs
procedural auxiliary and expected stderr. The inventory now has 26 cases and 51
unchanged source/output files. Both original licenses and COPYRIGHT bring the
retained inventory to 54 files.
An explicit native reference with Rust 1.99.0 observes the expected NonExistent
diagnostic after the attribute-generated Test/Bench declarations resolve. This
negative reference uses the original feature directives and is not compiletest or
LRR fixture acceptance. See artifacts/macos-first/m015-upstream-attribute-native-9iqqlyjr/report.json.


M016 now retains checked include producer chains and revalidated admitted include blobs in success and partial-failure diagnostics. Cold original declaration and intrinsic argument compiler demands use their exact hygienic selection and immutable lexical prefixes through the existing eager worklist. Bounded private argument snapshots omit only their own invocation position while leaving the live pending index unchanged, and statement-position include parses one expression as in the pinned rustc implementation; captures and record adoption remain explicit. Shared pinned grammar and declaration materialization reject ERROR syntax nodes even when the pinned parser diagnostic list is empty, so malformed or trailing tokens cannot be admitted as complete compiler output. Parser failures keep the actual opening-parenthesis API anchor and primary code/message without adopting output or a seal. Original function-like and derive requests forward their exact bound original occurrence and native definition header on actual matched-server panic, preserving prior attachment failures and cancelling the failed run. Procedural definition maps distinguish function-like, attribute and derive forms and capture only loaded exact exports. The renderer can accept bounded immutable snapshots keyed by exact unit, with content/range checks, conflict refusal, unit/snapshot disambiguation and no render-time source I/O. Compiler requests from included intrinsic arguments now require their exact admitted source view, including the actual span, source identity, origin/context and unchanged input leases; stale, missing or ambiguous included worlds refuse. Service supply of native external worlds and full schema forwarding remain pending. The final ancestry control executes exactly once with strict Clippy on its frozen candidate; the single-expression control passed on its separately frozen grammar-repair ancestor whose ancestry failure remains preserved. Four earlier include-capture/procedural/shadowing controls passed on their frozen ancestor whose two include failures remain preserved. Earlier header/renderer/line-map controls retain separately scoped evidence. These are separate snapshots and are not one combined all-six validation run. Unchanged official sources, auxiliaries, directives and expected output are retained at the recorded pin. No whole compiletest fixture passes are inferred. Full MX033/MX034, generated cold compiler source worlds, complete cache remapping, guest JIT, release and Linux remain pending.

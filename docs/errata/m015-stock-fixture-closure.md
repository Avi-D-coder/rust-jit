# E-M015-019: retain stock support inputs and rustfix expectations

Section57 requires unchanged official cases with their helpers, directives,
revisions, expected files, commit and hashes, under the original rustc license.
An import that silently omits a required input or expected file is incomplete,
even if the importer itself succeeds. It cannot establish fixture acceptance.

The original importer omitted expr_2021_cargo_fix_edition.fixed, despite the
unchanged primary's run-rustfix directive. It also lacked a way to retain the two
literal include-single-expr helpers without misclassifying them as independent
cases. Both problems reproduce against official commit
ae0fe511b15ab760d7e963b13389d1bc2ab42b18. No guest program was invented.
The collected baseline has one failed assertion, one argparse error and the
existing transitive procedural-auxiliary control passing. See
artifacts/macos-first/m015-stock-import-closure-baseline-report.json and log.

The private correction retains same-stem .fixed files alongside .stdout/.stderr
and adds an explicit --support-file input. Support files enter the checked file
inventory but not the case inventory. Existing bounded Git-blob/SHA/path/size
checks, original case auxiliary roots, license/COPYRIGHT bytes and merge refusal
remain in place. Supplemental files with auxiliary directives still require
an established owning case root; missing ownership fails rather than guessing a
root. No automatic include expansion, runtime query read, parser replacement,
native compilation or macro authority is added.

Three actual importer controls now pass on unchanged private tool inputs and
unchanged official source/output bytes: nested original auxiliary roots, include
helper roles with lossless re-import, and the complete Rust2021 edition-fragment
rustfix expectation. Retention: artifacts/macos-first/m015-stock-import-closure-retention.json.
These are fixture-closure checks. They are not execution of include diagnostics,
Rust's rustfix application, M016, full stock compiletest, guest JIT or release
acceptance. The correction and the two unchanged complete fixture closures are integrated after the prior main source hold was released. Inventory64 cases/131 original retained files; language acceptance remains pending. The current milestone after integration must pass before claiming task completion.

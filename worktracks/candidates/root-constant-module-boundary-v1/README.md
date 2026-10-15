# Constant primitive lookup module boundary

T024/T041 production repair for the V83 `LRR-E-CONST-SOURCE` failure in unchanged rustc `issue-21475.rs`. The primitive spelling scan for `m::START: u32` walked past module `m` into the root, where it incorrectly rejected `use m::{START, END}`. Rust unqualified names inside a module do not inherit its parent's imports or type declarations.

Inspect every enclosing function/block and the nearest owning ItemList/SourceFile, then stop. Local imports, producers, parameters and type shadows still refuse primitive fallback. The selected-source path still requires its private current owning Type certificate. This is not a permission to infer primitive identity from a runtime layout or from the printed type name alone.

The existing unchanged qualified-range main is the regression and retains its original results, constant queries and reached phases. Run it and existing constant shadow/import/parameter controls asynchronously before accepting this patch. Source replay and formatting are recorded; compilation and runtime acceptance are pending.

Validation update: V84 `e5742f/0` completes the unchanged original main in its default 2015 edition. Strict and existing constant regression scopes remain in progress; the integrated milestone has not passed.

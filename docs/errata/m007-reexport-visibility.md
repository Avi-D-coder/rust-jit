# E-M007-011: Imported macro visibility cannot widen

The first caller-aware lookup checked each reexport target from the importing
module's scope, but allowed a public reexport of an accessible private imported
alias. The native Rust 1.99.0 macOS compiler contradicted that behavior with
E0364. Retained failure: `artifacts/macos-first/m007-visibility-before-width-fix.log`.

Minimal witness:

```rust
#[macro_export] macro_rules! m { () => { 17 }; }
mod inner {
    use crate::m as private;
    pub use private as escaped; // E0364
}
```

Checking read access is necessary but insufficient. The target binding's declared
visibility must also contain the visibility exposed by the reexport. Public
contains every scope; a restricted scope contains only restrictions in its unit
and descendants. Reexports cannot widen private imported bindings merely because
the original macro definition was exported publicly.

`source_macro_binding_visibility_checks_callers_and_each_import_origin` now
expects the escaped alias to be inaccessible. The native matrix separately
requires the exact E0364 diagnostic and five E0603 private-access diagnostics.
No alternate frontend, profile or macro implementation is introduced.

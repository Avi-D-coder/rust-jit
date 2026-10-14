# E-M007-008: retain source spelling when validating raw macro names

The pinned AST's `Name::text` exposes the semantic name without `r#`. Passing
that string back through source-identifier validation rejects a valid 2024
`macro_rules! r#gen` declaration because bare `gen` is reserved. This was an
implementation contradiction, not a language-profile change.

The failing original-source regression is preserved in
`artifacts/macos-first/m007-source-index-before-raw-keyword-fix.log`. Textual
outline definition events now retain the exact pinned AST name syntax as well
as the existing semantic DefKey. MacroName validates the original spelling,
then canonicalizes its owned lookup identity. Tokens and source spans retain
the raw flag/spelling. The native macOS source-index test now selects original
`r#gen` definitions inside the earlier module and at the root, and expands their
actual selected source slices. Source-wide NFC bucket integration remains a
separate requirement.
